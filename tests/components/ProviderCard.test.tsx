import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import type { Provider } from "@/types";
import { ProviderCard } from "@/components/providers/ProviderCard";

vi.mock("@/lib/query/failover", () => ({
  useProviderHealth: () => ({ data: undefined }),
}));

vi.mock("@/lib/query/queries", () => ({
  useUsageQuery: () => ({ data: undefined }),
}));

function createProvider(overrides: Partial<Provider> = {}): Provider {
  return {
    id: overrides.id ?? "provider-1",
    name: overrides.name ?? "Provider One",
    settingsConfig: overrides.settingsConfig ?? {
      baseUrl: "https://api.example.com/v1",
    },
    category: overrides.category ?? "custom",
    meta: overrides.meta,
    websiteUrl: overrides.websiteUrl,
    notes: overrides.notes,
  };
}

function renderCard(
  props: Partial<ComponentProps<typeof ProviderCard>> = {},
) {
  const provider = props.provider ?? createProvider();
  const result = render(
    <ProviderCard
      provider={provider}
      isCurrent={props.isCurrent ?? false}
      appId={props.appId ?? "pi"}
      isInConfig={props.isInConfig ?? false}
      onSwitch={props.onSwitch ?? vi.fn()}
      onEdit={props.onEdit ?? vi.fn()}
      onDelete={props.onDelete ?? vi.fn()}
      onConfigureUsage={props.onConfigureUsage ?? vi.fn()}
      onOpenWebsite={props.onOpenWebsite ?? vi.fn()}
      onDuplicate={props.onDuplicate ?? vi.fn()}
      isTesting={props.isTesting ?? false}
      isProxyRunning={props.isProxyRunning ?? false}
      isProxyTakeover={props.isProxyTakeover ?? false}
      isDefaultModel={props.isDefaultModel}
      onSetAsDefault={props.onSetAsDefault}
    />,
  );

  const card = result.container.firstElementChild as HTMLElement;
  return { ...result, card };
}

describe("ProviderCard", () => {
  it("does not highlight a Pi provider merely because it is in live config", () => {
    const { card } = renderCard({
      appId: "pi",
      isInConfig: true,
      isDefaultModel: false,
    });

    expect(card.className).not.toContain("border-blue-500/60");
  });

  it("highlights the Pi default provider", () => {
    const { card } = renderCard({
      appId: "pi",
      isInConfig: true,
      isDefaultModel: true,
    });

    expect(card.className).toContain("border-blue-500/60");
  });

  it("keeps highlighting OpenCode providers that are in live config", () => {
    const { card } = renderCard({
      appId: "opencode",
      isInConfig: true,
      isDefaultModel: false,
    });

    expect(card.className).toContain("border-blue-500/60");
  });
});
