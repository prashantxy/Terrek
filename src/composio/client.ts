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

