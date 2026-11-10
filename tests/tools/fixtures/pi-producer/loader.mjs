// Resolve the writer's two value imports to their unchanged pinned modules.
import { registerHooks } from 'node:module';
const bridge = 'data:text/javascript,' + encodeURIComponent(`
export { getCurrentSystemMessage } from 'file:///producer/packages/ai/src/utils/transcript.ts';
export { uuidv7 } from 'file:///producer/packages/ai/src/utils/uuid.ts';
`);
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === '@earendil-works/pi-ai') return { url: bridge, shortCircuit: true };
    return nextResolve(specifier, context);
  },
});
