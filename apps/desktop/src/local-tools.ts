import catalog from '../../../crates/openusage-locallogs/sources/tools.json' with { type: 'json' };

export type LocalLogTool = keyof typeof catalog;
const allTools = (Object.keys(catalog) as LocalLogTool[]).sort((a, b) => catalog[a].order - catalog[b].order);
export const localLogTools = allTools.filter(tool => catalog[tool].enabled);
export const localLogToolNames = Object.fromEntries(allTools.map(tool => [tool, catalog[tool].name])) as Record<LocalLogTool, string>;
export const localLogToolTints = Object.fromEntries(allTools.map(tool => [tool, catalog[tool].tint])) as Record<LocalLogTool, number>;
export const orderedTools = (tools: LocalLogTool[]) => localLogTools.filter(tool => tools.includes(tool));
