import { Params, useParams } from "@solidjs/router";
import {
  AlreadyExistingForm,
  EditS3ConfigForm,
} from "../components/EditConfigForm";
import Layout from "../components/Layout";
import { Show, createResource } from "solid-js";
import { invoke } from "@tauri-apps/api/primitives";

export default function EditForm() {
  const params = useParams<{ id: string }>();
  const [data] = createResource(params, async ({ id }) => {
    const out: AlreadyExistingForm = await invoke("get_config", {
      id: Number(id),
    });
    return out;
  });

  return (
    <Layout>
      <Show when={data()}>
        <EditS3ConfigForm initialForm={data()} />
      </Show>
    </Layout>
  );
}
