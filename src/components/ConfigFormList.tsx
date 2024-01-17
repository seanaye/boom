import { invoke } from "@tauri-apps/api/primitives";
import { For, createResource } from "solid-js";
import { IconButton } from "./IconButton";

type Data = { id: number; bucket_name: string };

export function S3ConfigFormList() {
  const [formData] = createResource(async () => {
    const r: Array<Data> = await invoke("list_configs");
    return r;
  });
  const [active, { refetch }] = createResource(async () => {
    const r: { id: number } = await invoke("get_selected");
    return r;
  });

  async function onClick(configId: number) {
    await invoke("set_selected", { configId });
    refetch();
  }

  return (
    <div class="grid grid-cols-3 gap-4 justify-start">
      <For each={formData()}>
        {(d) => (
          <>
            <input
              type="checkbox"
              onclick={(e) => {
                e.preventDefault();
                onClick(d.id);
              }}
              id={`${d.id}`}
              checked={active()?.id === d.id}
            />
            <label for={`${d.id}`}>{d.bucket_name}</label>
            <IconButton as="a" href={`/settings/config/${d.id}/edit`}>
              <div class="i-heroicons-pencil" />
            </IconButton>
          </>
        )}
      </For>
    </div>
  );
}

// {/* <button */}
// {/*   onclick={() => onClick(d.id)} */}
// {/*   class={`p-2 border bg-gray-100 ${ */}
// {/*     active()?.id === d.id ? "bg-gray-200" : "" */}
// {/*   }`} */}
// {/* > */}
// {/*   {} */}
// {/* </button> */}
