import { invoke } from "@tauri-apps/api/primitives";
import { createStore } from "solid-js/store";

const defaultState = {
  nickname: "",
  endpoint: "",
  region: "",
  bucket_name: "",
  host_rewrite: "",
  public_key: "",
  private_key: "",
};

type FormState = typeof defaultState;

export type AlreadyExistingForm = FormState & { id: number };

export function EditS3ConfigForm(props: { initialForm?: AlreadyExistingForm }) {
  const [form, setForm] = createStore(props.initialForm ?? defaultState);

  const updateFormField = (fieldName: string) => (event: Event) => {
    const inputElement = event.currentTarget as HTMLInputElement;
    setForm({
      [fieldName]: inputElement.value,
    });
  };

  return (
    <form
      class="grid flow-col gap-4"
      onSubmit={async (e) => {
        e.preventDefault();
        const config = JSON.parse(JSON.stringify(form));
        console.log({ d: config });
        const res = await invoke("create_config", { config });
        console.log(res);
      }}
    >
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("nickname")}
          value={form.nickname}
        />
        Nickname
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("endpoint")}
          value={form.endpoint}
        />
        Endpoint
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("region")}
          value={form.region}
        />
        Region
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("bucket_name")}
          value={form.bucket_name}
        />
        Bucket Name
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("host_rewrite")}
          value={form.host_rewrite}
        />
        Host Rewrite (Optional)
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("public_key")}
          value={form.public_key}
        />
        Public Key
      </label>
      <label>
        <input
          type="password"
          placeholder="name"
          onChange={updateFormField("private_key")}
          value={form.private_key}
        />
        Private key
      </label>
      <button type="submit">{props.initialForm ? "Update" : "Create"}</button>
    </form>
  );
}
