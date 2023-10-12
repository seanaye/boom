import { invoke } from "@tauri-apps/api";
import { For, createResource } from "solid-js";

export function S3ConfigFormList() {
  const [formData] = createResource(async () => {
    const r: Array<{ id: number }> = await invoke("plugin:api|list_configs");
    return r;
  });
  const [active, { refetch }] = createResource(async () => {
    const r: { id: number } = await invoke("plugin:api|get_selected");
    return r;
  });

  async function onClick(configId: number) {
    await invoke("plugin:api|set_selected", { configId });
    refetch();
  }

  return (
    <div>
      <For each={formData()}>
        {(d) => (
          <button
            onclick={() => onClick(d.id)}
            class={`p-2 border bg-gray-100 ${
              active()?.id === d.id ? "bg-gray-200" : ""
            }`}
          >
            {JSON.stringify(d)}
          </button>
        )}
      </For>
    </div>
  );
}
