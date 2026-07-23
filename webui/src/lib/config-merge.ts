export interface LlmModelEntry {
  model: string;
  provider: string;
  weight: number;
  priority: number;
  toolCallMode?: 'native' | 'text' | 'none' | 'auto';
  temperature?: number;
  maxTokens?: number;
  inputPrice?: number;
  outputPrice?: number;
}

export function normalizeProviders(providers: Record<string, any>): Record<string, any> {
  return Object.fromEntries(Object.entries(providers).map(([key, value]) => {
    const proxy = value?.proxy;
    if (proxy == null || (typeof proxy === 'string' && proxy.trim() === '')) {
      const { proxy: _proxy, ...rest } = value ?? {};
      return [key, rest];
    }
    return [key, value];
  }));
}

export function mergeLlmConfig(
  latest: any,
  providers: Record<string, any>,
  modelPool: LlmModelEntry[],
): any {
  const defaults: any = {
    ...(latest?.agents?.defaults || {}),
    modelPool: modelPool.map((entry) => ({
      model: entry.model,
      provider: entry.provider,
      weight: entry.weight,
      priority: entry.priority,
      toolCallMode: entry.toolCallMode ?? 'native',
      temperature: entry.temperature,
      maxTokens: entry.maxTokens,
      inputPrice: entry.inputPrice,
      outputPrice: entry.outputPrice,
    })),
  };
  const primary = modelPool.find((entry) => entry.model.trim());
  if (primary) {
    defaults.model = primary.model.trim();
    defaults.provider = primary.provider?.trim() || defaults.provider;
  }
  return {
    ...latest,
    providers: normalizeProviders(providers),
    agents: { ...(latest?.agents || {}), defaults },
  };
}
