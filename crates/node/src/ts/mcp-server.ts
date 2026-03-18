// MCP server exposing astral analysis as tools
// Uses @modelcontextprotocol/sdk

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from '@modelcontextprotocol/sdk/types.js';

import * as path from 'node:path';

// ---------------------------------------------------------------------------
// Native binding – loaded at runtime from the compiled napi addon.
// The require path resolves to the package root's index.js which re-exports
// the platform-specific binary.  This will throw at import time if the
// native addon has not been built yet (`npm run build`).
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-var-requires
const { Analyser } = require('../../index.js') as {
  Analyser: new (configJson: string) => AnalyserBinding;
};

interface AnalyserBinding {
  scan(repoPath: string): CodeChunk[];
  buildRequests(repoPath: string): string;
  aggregateResults(repoPath: string, jsonl: string): string;
  renderOutput(resultsJson: string, format: string): string;
}

interface CodeChunk {
  id: string;
  filePath: string;
  language: string;
  chunkType: string;
  name: string | null;
  content: string;
  startLine: number;
  endLine: number;
  imports: string[];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeAnalyser(config?: Record<string, unknown>): AnalyserBinding {
  const configJson = config ? JSON.stringify(config) : '{}';
  return new Analyser(configJson);
}

/** Rough token estimate: ~4 chars per token. */
function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

// ---------------------------------------------------------------------------
// MCP Server
// ---------------------------------------------------------------------------

const server = new Server(
  {
    name: 'astral',
    version: '0.1.0',
  },
  {
    capabilities: {
      tools: {},
    },
  },
);

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: 'astral_scan',
      description:
        'Scan a repository and return code chunks. Each chunk contains a file path, language, type, name, content, line range, and imports.',
      inputSchema: {
        type: 'object' as const,
        properties: {
          repo_path: {
            type: 'string',
            description: 'Absolute or relative path to the repository to scan',
          },
          config: {
            type: 'object',
            description: 'Optional astral configuration object',
          },
        },
        required: ['repo_path'],
      },
    },
    {
      name: 'astral_analyse',
      description:
        'Run the full analysis pipeline (scan + build batch requests). Returns batch request JSON ready for submission to the Anthropic Batch API.',
      inputSchema: {
        type: 'object' as const,
        properties: {
          repo_path: {
            type: 'string',
            description: 'Absolute or relative path to the repository to analyse',
          },
          mode: {
            type: 'string',
            description:
              'Analysis mode – e.g. "summarise", "code_review", "security_audit", "test_generation", "doc_generation"',
          },
          config: {
            type: 'object',
            description: 'Optional astral configuration object',
          },
        },
        required: ['repo_path'],
      },
    },
    {
      name: 'astral_render',
      description:
        'Render analysis results to a specified format (markdown, json, csv, html).',
      inputSchema: {
        type: 'object' as const,
        properties: {
          results_json: {
            type: 'string',
            description: 'JSON string of aggregated analysis results',
          },
          format: {
            type: 'string',
            description: 'Output format: "markdown", "json", "csv", or "html"',
          },
        },
        required: ['results_json', 'format'],
      },
    },
    {
      name: 'astral_estimate_cost',
      description:
        'Estimate the batch API cost for analysing a repository. Returns token counts and an approximate USD cost.',
      inputSchema: {
        type: 'object' as const,
        properties: {
          repo_path: {
            type: 'string',
            description: 'Absolute or relative path to the repository',
          },
          config: {
            type: 'object',
            description: 'Optional astral configuration object',
          },
        },
        required: ['repo_path'],
      },
    },
  ],
}));

// ---------------------------------------------------------------------------
// Tool handlers
// ---------------------------------------------------------------------------

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    switch (name) {
      // -------------------------------------------------------------------
      // astral_scan
      // -------------------------------------------------------------------
      case 'astral_scan': {
        const repoPath = path.resolve(args?.repo_path as string);
        const config = args?.config as Record<string, unknown> | undefined;
        const analyser = makeAnalyser(config);
        const chunks = analyser.scan(repoPath);

        const summaries = chunks.map((c) => ({
          id: c.id,
          filePath: c.filePath,
          language: c.language,
          chunkType: c.chunkType,
          name: c.name,
          startLine: c.startLine,
          endLine: c.endLine,
          importCount: c.imports.length,
          contentLength: c.content.length,
        }));

        return {
          content: [
            {
              type: 'text' as const,
              text: JSON.stringify(
                { chunkCount: summaries.length, chunks: summaries },
                null,
                2,
              ),
            },
          ],
        };
      }

      // -------------------------------------------------------------------
      // astral_analyse
      // -------------------------------------------------------------------
      case 'astral_analyse': {
        const repoPath = path.resolve(args?.repo_path as string);
        const mode = args?.mode as string | undefined;
        const config = (args?.config as Record<string, unknown>) ?? {};
        if (mode) {
          config.analysis_mode = mode;
        }

        const analyser = makeAnalyser(config);

        // Scan first to give a chunk count
        const chunks = analyser.scan(repoPath);
        const requestsJson = analyser.buildRequests(repoPath);
        const requests = JSON.parse(requestsJson);

        return {
          content: [
            {
              type: 'text' as const,
              text: JSON.stringify(
                {
                  chunkCount: chunks.length,
                  requestCount: requests.length,
                  model: requests[0]?.params?.model ?? 'N/A',
                  requests,
                },
                null,
                2,
              ),
            },
          ],
        };
      }

      // -------------------------------------------------------------------
      // astral_render
      // -------------------------------------------------------------------
      case 'astral_render': {
        const resultsJson = args?.results_json as string;
        const format = args?.format as string;

        // Create a throwaway Analyser just for rendering – config is unused
        const analyser = makeAnalyser();
        const rendered = analyser.renderOutput(resultsJson, format);

        return {
          content: [
            {
              type: 'text' as const,
              text: rendered,
            },
          ],
        };
      }

      // -------------------------------------------------------------------
      // astral_estimate_cost
      // -------------------------------------------------------------------
      case 'astral_estimate_cost': {
        const repoPath = path.resolve(args?.repo_path as string);
        const config = args?.config as Record<string, unknown> | undefined;
        const analyser = makeAnalyser(config);

        const chunks = analyser.scan(repoPath);
        const requestsJson = analyser.buildRequests(repoPath);
        const requests = JSON.parse(requestsJson);

        // Estimate tokens from the request payloads
        let totalInputTokens = 0;
        for (const req of requests) {
          const systemTokens = req.params.system
            ? estimateTokens(req.params.system)
            : 0;
          const messageTokens = (
            req.params.messages as Array<{ content: string }>
          ).reduce((sum: number, m: { content: string }) => sum + estimateTokens(m.content), 0);
          totalInputTokens += systemTokens + messageTokens;
        }

        // Assume max_tokens as estimated output (conservative upper bound)
        const maxOutputPerRequest = requests[0]?.params?.max_tokens ?? 4096;
        const totalOutputTokens = requests.length * maxOutputPerRequest;

        // Batch API pricing: 50% of standard pricing
        // Claude 3.5 Sonnet: $3/1M input, $15/1M output (standard)
        // Batch: $1.50/1M input, $7.50/1M output
        const inputCost = (totalInputTokens / 1_000_000) * 1.5;
        const outputCost = (totalOutputTokens / 1_000_000) * 7.5;
        const estimatedCostUsd = inputCost + outputCost;

        return {
          content: [
            {
              type: 'text' as const,
              text: JSON.stringify(
                {
                  chunkCount: chunks.length,
                  requestCount: requests.length,
                  estimatedInputTokens: totalInputTokens,
                  estimatedMaxOutputTokens: totalOutputTokens,
                  estimatedCostUsd: Math.round(estimatedCostUsd * 1000) / 1000,
                  note: 'Output token estimate uses max_tokens as upper bound; actual cost is typically much lower.',
                },
                null,
                2,
              ),
            },
          ],
        };
      }

      // -------------------------------------------------------------------
      // Unknown tool
      // -------------------------------------------------------------------
      default:
        return {
          content: [
            {
              type: 'text' as const,
              text: `Unknown tool: ${name}`,
            },
          ],
          isError: true,
        };
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      content: [
        {
          type: 'text' as const,
          text: `Error executing ${name}: ${message}`,
        },
      ],
      isError: true,
    };
  }
});

// ---------------------------------------------------------------------------
// Start the server
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  // Server is now listening on stdin/stdout via the MCP stdio transport.
  // It will remain running until the transport is closed.
}

main().catch((error) => {
  console.error('Fatal: MCP server failed to start:', error);
  process.exit(1);
});
