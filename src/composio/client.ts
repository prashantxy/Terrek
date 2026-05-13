import "dotenv/config";
import {Composio} from "@composio/core"
import { Agent, run, MemorySession } from "@openai/agents";
import {OpenAIAgentsProvider} from "@composio/openai-agents";
import { createInterface } from "readline/promises";


const composio = new Composio({
    provider: new OpenAIAgentsProvider()
});

const userID="moralizer_001";
const session = await composio.create(userID);
const tools = await session.tools();

// For multi-turn, store the session ID in your db and reuse instead of calling create() again:
// const sessionId = session.sessionId;
// const session = await composio.use(sessionId);

const agent = new Agent({
  name: "Personal Assistant",
  instructions: "You are a helpful personal assistant. Use Composio tools to take action.",
  model: "opus",
  tools,
});
const memory = new MemorySession();
const readline = createInterface({ input: process.stdin, output: process.stdout });
console.log(`
What task would you like me to help you with?
I can use tools like Gmail, GitHub, Linear, Notion, and more.
(Type 'exit' to exit)
Example tasks:
  - 'Summarize my emails from today'
  - 'List all open issues on the composio github repository'
`);
while (true) {
  const input = (await readline.question("You: ")).trim();
  if (input.toLowerCase() === "exit") break;
  process.stdout.write("Assistant: ");
  const result = await run(agent, input, { session: memory });
  process.stdout.write(`${result.finalOutput}\n`);
}
readline.close();
