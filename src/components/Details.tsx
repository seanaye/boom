import { JSX, Show, createSignal } from "solid-js";

/**
 * Wrapper over browser native which lazily mounts the contents
 */
export function Details(
  props: JSX.DetailsHtmlAttributes<HTMLDetailsElement> & {
    summary: JSX.Element;
  },
) {
  const [open, setOpen] = createSignal(false);

  return (
    <details
      {...props}
      open={open()}
      onToggle={(e) => {
        setOpen(e.currentTarget.open);
      }}
    >
      {props.summary}
      <Show when={open}>{props.children}</Show>
    </details>
  );
}
