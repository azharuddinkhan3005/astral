import type { Agent, AgentResult, AgentContext } from "./agents/index.js";

// ---------------------------------------------------------------------------
// Schema validation helpers
// ---------------------------------------------------------------------------

interface SchemaField {
  name: string;
  type: "string" | "number" | "boolean" | "object" | "array";
  required: boolean;
}

function validateSchema(
  data: Record<string, unknown>,
  fields: SchemaField[]
): string[] {
  const errors: string[] = [];
  for (const field of fields) {
    if (field.required && !(field.name in data)) {
      errors.push(`Missing required field: ${field.name}`);
      continue;
    }
    if (field.name in data) {
      const val = data[field.name];
      const actual = Array.isArray(val) ? "array" : typeof val;
      if (actual !== field.type) {
        errors.push(
          `Field "${field.name}": expected ${field.type}, got ${actual}`
        );
      }
    }
  }
  return errors;
}

// ---------------------------------------------------------------------------
// Cost tracking
// ---------------------------------------------------------------------------

export interface CostEntry {
  agentId: string;
  taskId: string;
  inputTokens: number;
  outputTokens: number;
  estimatedCostUsd: number;
  timestamp: number;
}

// ---------------------------------------------------------------------------
// Log entry
// ---------------------------------------------------------------------------

export interface LogEntry {
  level: "debug" | "info" | "warn" | "error";
  agentId: string;
  taskId: string;
  message: string;
  timestamp: number;
  metadata?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Task graph
// ---------------------------------------------------------------------------

export interface PipelineTask {
  id: string;
  agentId: string;
  input: Record<string, unknown>;
  /** Task IDs this task depends on. Empty = ready to run. */
  dependsOn: string[];
}

interface TaskState {
  task: PipelineTask;
  status: "pending" | "running" | "completed" | "failed";
  result?: AgentResult;
  error?: string;
}

// ---------------------------------------------------------------------------
// AgentRuntime
// ---------------------------------------------------------------------------

export class AgentRuntime {
  private agents = new Map<string, Agent>();
  private logs: LogEntry[] = [];
  private costs: CostEntry[] = [];

  // -----------------------------------------------------------------------
  // Agent registry
  // -----------------------------------------------------------------------

  registerAgent(agent: Agent): void {
    this.agents.set(agent.id, agent);
  }

  getAgent(id: string): Agent | undefined {
    return this.agents.get(id);
  }

  listAgents(): Agent[] {
    return Array.from(this.agents.values());
  }

  // -----------------------------------------------------------------------
  // Single dispatch with schema validation
  // -----------------------------------------------------------------------

  async dispatch(
    agentId: string,
    input: Record<string, unknown>,
    context: AgentContext
  ): Promise<AgentResult> {
    const agent = this.agents.get(agentId);
    if (!agent) {
      throw new Error(`Agent not found: ${agentId}`);
    }

    // Validate input schema
    if (agent.inputSchema) {
      const errors = validateSchema(input, agent.inputSchema);
      if (errors.length > 0) {
        throw new Error(
          `Schema validation failed for agent "${agentId}": ${errors.join("; ")}`
        );
      }
    }

    this.log("info", agentId, context.taskId ?? "dispatch", "Dispatching agent");

    const start = Date.now();
    const result = await agent.run(input, context);
    const elapsed = Date.now() - start;

    // Track cost if provided
    if (result.cost) {
      this.costs.push({
        agentId,
        taskId: context.taskId ?? "dispatch",
        inputTokens: result.cost.inputTokens,
        outputTokens: result.cost.outputTokens,
        estimatedCostUsd: result.cost.estimatedCostUsd,
        timestamp: Date.now(),
      });
    }

    this.log(
      "info",
      agentId,
      context.taskId ?? "dispatch",
      `Agent completed in ${elapsed}ms`,
      { elapsed, status: result.status }
    );

    return result;
  }

  // -----------------------------------------------------------------------
  // Pipeline execution with parallel dispatch
  // -----------------------------------------------------------------------

  async runPipeline(
    tasks: PipelineTask[],
    context: Omit<AgentContext, "taskId" | "previousResults">
  ): Promise<Map<string, AgentResult>> {
    const states = new Map<string, TaskState>();
    for (const task of tasks) {
      states.set(task.id, { task, status: "pending" });
    }

    const results = new Map<string, AgentResult>();

    // Keep looping until all tasks are completed or failed
    // eslint-disable-next-line no-constant-condition
    while (true) {
      // Find tasks that are ready (all deps completed)
      const ready: TaskState[] = [];
      for (const state of states.values()) {
        if (state.status !== "pending") continue;
        const depsComplete = state.task.dependsOn.every((dep) => {
          const depState = states.get(dep);
          return depState?.status === "completed";
        });
        const depsFailed = state.task.dependsOn.some((dep) => {
          const depState = states.get(dep);
          return depState?.status === "failed";
        });
        if (depsFailed) {
          state.status = "failed";
          state.error = "Dependency failed";
          this.log(
            "error",
            state.task.agentId,
            state.task.id,
            "Skipped: dependency failed"
          );
          continue;
        }
        if (depsComplete) {
          ready.push(state);
        }
      }

      if (ready.length === 0) {
        // Check if there are still pending tasks (cycle detection)
        const pending = Array.from(states.values()).filter(
          (s) => s.status === "pending"
        );
        if (pending.length > 0) {
          this.log(
            "error",
            "runtime",
            "pipeline",
            `Deadlock: ${pending.length} tasks pending with unresolvable deps`
          );
          for (const p of pending) {
            p.status = "failed";
            p.error = "Deadlocked — circular dependency";
          }
        }
        break;
      }

      // Dispatch ready tasks in parallel
      const promises = ready.map(async (state) => {
        state.status = "running";

        // Collect previous results from dependencies
        const previousResults: Record<string, AgentResult> = {};
        for (const dep of state.task.dependsOn) {
          const depResult = results.get(dep);
          if (depResult) {
            previousResults[dep] = depResult;
          }
        }

        try {
          const result = await this.dispatch(state.task.agentId, state.task.input, {
            ...context,
            taskId: state.task.id,
            previousResults,
          });
          state.status = "completed";
          state.result = result;
          results.set(state.task.id, result);
        } catch (err) {
          state.status = "failed";
          state.error = err instanceof Error ? err.message : String(err);
          this.log(
            "error",
            state.task.agentId,
            state.task.id,
            `Agent failed: ${state.error}`
          );
        }
      });

      await Promise.all(promises);
    }

    return results;
  }

  // -----------------------------------------------------------------------
  // Logging
  // -----------------------------------------------------------------------

  private log(
    level: LogEntry["level"],
    agentId: string,
    taskId: string,
    message: string,
    metadata?: Record<string, unknown>
  ): void {
    this.logs.push({
      level,
      agentId,
      taskId,
      message,
      timestamp: Date.now(),
      metadata,
    });
  }

  getLogs(): ReadonlyArray<LogEntry> {
    return this.logs;
  }

  // -----------------------------------------------------------------------
  // Cost tracking
  // -----------------------------------------------------------------------

  getCosts(): ReadonlyArray<CostEntry> {
    return this.costs;
  }

  getTotalCost(): number {
    return this.costs.reduce((sum, c) => sum + c.estimatedCostUsd, 0);
  }
}
