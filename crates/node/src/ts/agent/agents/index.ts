// ---------------------------------------------------------------------------
// Agent contract
// ---------------------------------------------------------------------------

export interface SchemaField {
  name: string;
  type: "string" | "number" | "boolean" | "object" | "array";
  required: boolean;
}

export interface AgentCost {
  inputTokens: number;
  outputTokens: number;
  estimatedCostUsd: number;
}

export interface AgentResult {
  status: "ok" | "error";
  data: Record<string, unknown>;
  cost?: AgentCost;
}

export interface AgentContext {
  repoPath: string;
  configJson: string;
  taskId?: string;
  previousResults?: Record<string, AgentResult>;
}

export interface Agent {
  id: string;
  name: string;
  description: string;
  inputSchema?: SchemaField[];
  run(input: Record<string, unknown>, context: AgentContext): Promise<AgentResult>;
}

// ---------------------------------------------------------------------------
// Built-in agents
// ---------------------------------------------------------------------------

/**
 * Orchestrator agent — plans and coordinates multi-step analysis pipelines.
 * Given a high-level goal it determines which sub-agents to invoke and in
 * what order, then collects and merges their results.
 */
export const orchestratorAgent: Agent = {
  id: "orchestrator",
  name: "Orchestrator",
  description:
    "Plans analysis pipelines and coordinates sub-agent execution order",
  inputSchema: [
    { name: "goal", type: "string", required: true },
    { name: "repoPath", type: "string", required: true },
  ],
  async run(input, context) {
    const goal = input["goal"] as string;
    const repoPath = (input["repoPath"] as string) ?? context.repoPath;

    // Determine pipeline steps based on the goal keywords
    const steps: string[] = [];

    if (/review|audit|quality/i.test(goal)) {
      steps.push("walker", "parser", "reviewer");
    } else if (/doc|document/i.test(goal)) {
      steps.push("walker", "parser", "documenter");
    } else if (/security|vuln/i.test(goal)) {
      steps.push("walker", "parser", "security_auditor");
    } else if (/test/i.test(goal)) {
      steps.push("walker", "parser", "test_generator");
    } else {
      // Default: summarise
      steps.push("walker", "parser", "summariser");
    }

    return {
      status: "ok",
      data: {
        plan: steps,
        repoPath,
        goal,
        message: `Planned ${steps.length}-step pipeline: ${steps.join(" -> ")}`,
      },
    };
  },
};

/**
 * Walker agent — scans a repository and returns a list of discovered files
 * with language detection metadata.
 */
export const walkerAgent: Agent = {
  id: "walker",
  name: "Walker",
  description: "Scans a repository tree and returns discovered source files",
  inputSchema: [{ name: "repoPath", type: "string", required: true }],
  async run(input, context) {
    // Delegate to the native Analyser.scan() for file discovery
    const { Analyser } = require("../../../../index.js") as {
      Analyser: new (cfg: string) => { scan(p: string): unknown[] };
    };
    const analyser = new Analyser(context.configJson);
    const repoPath = (input["repoPath"] as string) ?? context.repoPath;
    const chunks = analyser.scan(repoPath);

    // Deduplicate file paths from chunks
    const fileSet = new Set<string>();
    for (const chunk of chunks as Array<{ filePath: string; language: string }>) {
      fileSet.add(chunk.filePath);
    }

    return {
      status: "ok",
      data: {
        files: Array.from(fileSet),
        fileCount: fileSet.size,
        chunkCount: chunks.length,
      },
    };
  },
};

/**
 * Parser agent — takes the scanned chunks and organises them into a
 * structured representation suitable for downstream analysis agents.
 */
export const parserAgent: Agent = {
  id: "parser",
  name: "Parser",
  description:
    "Parses scanned files into structured code chunks for analysis",
  inputSchema: [{ name: "repoPath", type: "string", required: true }],
  async run(input, context) {
    const { Analyser } = require("../../../../index.js") as {
      Analyser: new (cfg: string) => { scan(p: string): unknown[] };
    };
    const analyser = new Analyser(context.configJson);
    const repoPath = (input["repoPath"] as string) ?? context.repoPath;
    const chunks = analyser.scan(repoPath);

    // Group chunks by file
    const byFile = new Map<string, unknown[]>();
    for (const chunk of chunks as Array<{ filePath: string }>) {
      const list = byFile.get(chunk.filePath) ?? [];
      list.push(chunk);
      byFile.set(chunk.filePath, list);
    }

    return {
      status: "ok",
      data: {
        chunks,
        chunkCount: chunks.length,
        fileCount: byFile.size,
        fileMap: Object.fromEntries(byFile),
      },
    };
  },
};

/**
 * Summariser agent — generates concise summaries for each code chunk by
 * building batch requests and returning them for submission.
 */
export const summariserAgent: Agent = {
  id: "summariser",
  name: "Summariser",
  description: "Generates concise summaries for code chunks",
  inputSchema: [{ name: "repoPath", type: "string", required: true }],
  async run(input, context) {
    const { Analyser } = require("../../../../index.js") as {
      Analyser: new (cfg: string) => { buildRequests(p: string): string };
    };
    const config = JSON.parse(context.configJson || "{}");
    config.analysis_mode = "summarise";
    const analyser = new Analyser(JSON.stringify(config));
    const repoPath = (input["repoPath"] as string) ?? context.repoPath;
    const requestsJson = analyser.buildRequests(repoPath);
    const requests = JSON.parse(requestsJson);

    return {
      status: "ok",
      data: {
        requests,
        requestCount: requests.length,
        mode: "summarise",
      },
    };
  },
};

/**
 * Reviewer agent — performs code review analysis on each code chunk.
 */
export const reviewerAgent: Agent = {
  id: "reviewer",
  name: "Reviewer",
  description: "Performs code review on all code chunks",
  inputSchema: [{ name: "repoPath", type: "string", required: true }],
  async run(input, context) {
    const { Analyser } = require("../../../../index.js") as {
      Analyser: new (cfg: string) => { buildRequests(p: string): string };
    };
    const config = JSON.parse(context.configJson || "{}");
    config.analysis_mode = "code_review";
    const analyser = new Analyser(JSON.stringify(config));
    const repoPath = (input["repoPath"] as string) ?? context.repoPath;
    const requestsJson = analyser.buildRequests(repoPath);
    const requests = JSON.parse(requestsJson);

    return {
      status: "ok",
      data: {
        requests,
        requestCount: requests.length,
        mode: "code_review",
      },
    };
  },
};

/**
 * Security auditor agent — runs a security-focused analysis pass.
 */
export const securityAuditorAgent: Agent = {
  id: "security_auditor",
  name: "Security Auditor",
  description: "Runs security audit on all code chunks",
  inputSchema: [{ name: "repoPath", type: "string", required: true }],
  async run(input, context) {
    const { Analyser } = require("../../../../index.js") as {
      Analyser: new (cfg: string) => { buildRequests(p: string): string };
    };
    const config = JSON.parse(context.configJson || "{}");
    config.analysis_mode = "security_audit";
    const analyser = new Analyser(JSON.stringify(config));
    const repoPath = (input["repoPath"] as string) ?? context.repoPath;
    const requestsJson = analyser.buildRequests(repoPath);
    const requests = JSON.parse(requestsJson);

    return {
      status: "ok",
      data: {
        requests,
        requestCount: requests.length,
        mode: "security_audit",
      },
    };
  },
};

/**
 * Test generator agent — generates test cases for each code chunk.
 */
export const testGeneratorAgent: Agent = {
  id: "test_generator",
  name: "Test Generator",
  description: "Generates unit tests for code chunks",
  inputSchema: [{ name: "repoPath", type: "string", required: true }],
  async run(input, context) {
    const { Analyser } = require("../../../../index.js") as {
      Analyser: new (cfg: string) => { buildRequests(p: string): string };
    };
    const config = JSON.parse(context.configJson || "{}");
    config.analysis_mode = "test_generation";
    const analyser = new Analyser(JSON.stringify(config));
    const repoPath = (input["repoPath"] as string) ?? context.repoPath;
    const requestsJson = analyser.buildRequests(repoPath);
    const requests = JSON.parse(requestsJson);

    return {
      status: "ok",
      data: {
        requests,
        requestCount: requests.length,
        mode: "test_generation",
      },
    };
  },
};

/**
 * Documenter agent — generates inline documentation for code chunks.
 */
export const documenterAgent: Agent = {
  id: "documenter",
  name: "Documenter",
  description: "Generates inline documentation for code chunks",
  inputSchema: [{ name: "repoPath", type: "string", required: true }],
  async run(input, context) {
    const { Analyser } = require("../../../../index.js") as {
      Analyser: new (cfg: string) => { buildRequests(p: string): string };
    };
    const config = JSON.parse(context.configJson || "{}");
    config.analysis_mode = "doc_generation";
    const analyser = new Analyser(JSON.stringify(config));
    const repoPath = (input["repoPath"] as string) ?? context.repoPath;
    const requestsJson = analyser.buildRequests(repoPath);
    const requests = JSON.parse(requestsJson);

    return {
      status: "ok",
      data: {
        requests,
        requestCount: requests.length,
        mode: "doc_generation",
      },
    };
  },
};

// ---------------------------------------------------------------------------
// Registry helper — returns all built-in agents
// ---------------------------------------------------------------------------

export const builtinAgents: Agent[] = [
  orchestratorAgent,
  walkerAgent,
  parserAgent,
  summariserAgent,
  reviewerAgent,
  securityAuditorAgent,
  testGeneratorAgent,
  documenterAgent,
];
