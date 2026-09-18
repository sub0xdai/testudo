import net from "node:net";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const TOOLS = [
  {
    "description": "Ask the user to choose from Lab objects or next actions. Use for ambiguous environments, configs, runs, evals, models, workspaces, or launch choices.",
    "label": "Lab choose",
    "name": "choose",
    "parameters": {
      "additionalProperties": true,
      "properties": {
        "allow_multiple": {
          "type": "boolean"
        },
        "candidates": {
          "items": {
            "type": "object"
          },
          "type": "array"
        },
        "default_id": {
          "type": "string"
        },
        "prompt": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "candidates"
      ],
      "type": "object"
    }
  },
  {
    "description": "Search platform environments through the Prime CLI. Use before creating eval or training configs when the user names an environment.",
    "label": "Lab search environments",
    "name": "search_environments",
    "parameters": {
      "additionalProperties": true,
      "properties": {
        "limit": {
          "type": "integer"
        },
        "query": {
          "type": "string"
        }
      },
      "required": [],
      "type": "object"
    }
  },
  {
    "description": "Open a native hosted-training config editor from explicit training fields. Use this for RL training requests after resolving an environment with `search_environments`.",
    "label": "Lab train model",
    "name": "train_model",
    "parameters": {
      "additionalProperties": true,
      "properties": {
        "batch_size": {
          "minimum": 1,
          "type": "integer"
        },
        "env": {
          "description": "Platform environment in owner/name form.",
          "pattern": "^[^/]+/[^/]+$",
          "type": "string"
        },
        "max_steps": {
          "minimum": 1,
          "type": "integer"
        },
        "max_tokens": {
          "minimum": 1,
          "type": "integer"
        },
        "model": {
          "type": "string"
        },
        "rollouts_per_example": {
          "minimum": 1,
          "type": "integer"
        }
      },
      "required": [
        "env",
        "model",
        "max_steps",
        "batch_size",
        "rollouts_per_example",
        "max_tokens"
      ],
      "type": "object"
    }
  },
  {
    "description": "Open a native Lab config editor. Use when the user asks to create, tweak, clone, rerun, evaluate, train, or modify a config.",
    "label": "Lab edit config",
    "name": "edit_config",
    "parameters": {
      "additionalProperties": true,
      "properties": {
        "config": {
          "type": "object"
        },
        "config_kind": {
          "enum": [
            "eval",
            "rl",
            "gepa"
          ],
          "type": "string"
        },
        "defaults": {
          "type": "object"
        },
        "editable_fields": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "read_only_fields": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "source": {
          "type": "object"
        },
        "validation": {
          "type": "object"
        }
      },
      "required": [
        "title",
        "config_kind"
      ],
      "type": "object"
    }
  },
  {
    "description": "Show a side-effect preview with validation and confirm/cancel controls. Use before writes, syncs, installs, launches, pushes, deletes, or agent-applied remediations.",
    "label": "Lab preview action",
    "name": "preview_action",
    "parameters": {
      "additionalProperties": true,
      "properties": {
        "actions": {
          "items": {
            "type": "object"
          },
          "type": "array"
        },
        "requires_confirmation": {
          "type": "boolean"
        },
        "side_effects": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "validation": {
          "type": "object"
        }
      },
      "required": [
        "title",
        "actions"
      ],
      "type": "object"
    }
  },
  {
    "description": "Launch or rerun an evaluation/training job from a selected config. Use when the user is ready to run inside Lab and should see a native launch preview plus live-log handoff.",
    "label": "Lab launch run",
    "name": "launch_run",
    "parameters": {
      "additionalProperties": true,
      "properties": {
        "command": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "config": {
          "type": "object"
        },
        "config_kind": {
          "enum": [
            "eval",
            "rl",
            "gepa"
          ],
          "type": "string"
        },
        "config_path": {
          "type": "string"
        },
        "source_run_id": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "config_kind"
      ],
      "type": "object"
    }
  },
  {
    "description": "Show file changes or proposed source edits. Use after creating/editing environment code, configs, README content, skills, docs, or setup assets.",
    "label": "Lab show patch",
    "name": "show_patch",
    "parameters": {
      "additionalProperties": true,
      "properties": {
        "files": {
          "items": {
            "type": "object"
          },
          "type": "array"
        },
        "next_actions": {
          "items": {
            "type": "object"
          },
          "type": "array"
        },
        "risk_notes": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "summary": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "files"
      ],
      "type": "object"
    }
  },
  {
    "description": "Open rollout/eval sample inspection with selected failures, metrics, or examples. Use when diagnosing training/eval behavior or summarizing qualitative issues.",
    "label": "Lab inspect rollouts",
    "name": "inspect_rollouts",
    "parameters": {
      "additionalProperties": true,
      "properties": {
        "eval_id": {
          "type": "string"
        },
        "failure_categories": {
          "items": {
            "type": "object"
          },
          "type": "array"
        },
        "filters": {
          "type": "object"
        },
        "proposed_next_action": {
          "type": "object"
        },
        "run_id": {
          "type": "string"
        },
        "sample_ids": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "title"
      ],
      "type": "object"
    }
  }
];

function labSocketPath(cwd: string): string {
  const workspace = path.resolve(cwd);
  const digest = crypto.createHash("sha256").update(workspace).digest("hex").slice(0, 24);
  const root = process.env.PRIME_LAB_RUNTIME_DIR || os.tmpdir();
  return path.join(root, `prime-lab-${os.userInfo().uid}`, digest, "lab.sock");
}

function callLab(
  workspace: string,
  tool: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  const socketPath = labSocketPath(workspace);
  return new Promise((resolve, reject) => {
    const client = net.createConnection(socketPath);
    let data = "";
    client.setTimeout(5000);
    client.on("connect", () => {
      client.write(JSON.stringify({
        request_id: crypto.randomUUID(),
        tool,
        arguments: args || {},
      }) + "\n");
    });
    client.on("data", chunk => {
      data += chunk.toString("utf8");
      if (data.includes("\n")) {
        client.end();
      }
    });
    client.on("timeout", () => {
      client.destroy(new Error("Prime Lab IPC timed out."));
    });
    client.on("error", reject);
    client.on("close", () => {
      if (!data.trim()) {
        reject(new Error("Prime Lab IPC returned no response."));
        return;
      }
      try {
        const response = JSON.parse(data.trim());
        if (!response.ok) {
          reject(new Error(String(response.error || "Prime Lab tool call failed.")));
          return;
        }
        resolve(response.result);
      } catch (error) {
        reject(error);
      }
    });
  });
}

export default function primeLabExtension(pi: ExtensionAPI) {
  for (const tool of TOOLS) {
    pi.registerTool({
      name: tool.name,
      label: tool.label,
      description: tool.description,
      parameters: tool.parameters as any,
      async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
        const result = await callLab(ctx.cwd, tool.name, params as Record<string, unknown>);
        return {
          content: [{ type: "text", text: JSON.stringify(result) }],
          details: result,
        };
      },
    });
  }
}
