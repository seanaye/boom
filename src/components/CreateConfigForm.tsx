import { invoke } from "@tauri-apps/api";
import { createStore } from "solid-js/store";

export function CreateS3ConfigForm() {
  const [form, setForm] = createStore({
    nickname: "",
    endpoint: "",
    region: "",
    bucket_name: "",
    host_rewrite: "",
    public_key: "",
    private_key: "",
  });

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
        const res = await invoke("plugin:api|create_config", { config });
        console.log(res);
      }}
    >
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("nickname")}
        />
        Nickname
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("endpoint")}
        />
        Endpoint
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("region")}
        />
        Region
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("bucket_name")}
        />
        Bucket Name
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("host_rewrite")}
        />
        Host Rewrite (Optional)
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("public_key")}
        />
        Public Key
      </label>
      <label>
        <input
          type="password"
          placeholder="name"
          onChange={updateFormField("private_key")}
        />
        Private key
      </label>
      <button type="submit">Create</button>
    </form>
  );
}
