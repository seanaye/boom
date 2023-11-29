import { JSX, Show, createSignal } from "solid-js";

/**
 * Wrapper over browser native which lazily mounts the contents
 */
export function Details(
  props: JSX.DetailsHtmlAttributes<HTMLDetailsElement> & {
    summary: JSX.Element;
  },
) {
  const { summary, ...rest } = props;
  const [open, setOpen] = createSignal(false);

  return (
    <details
      {...rest}
      open={open()}
      onToggle={(e) => {
        setOpen(e.currentTarget.open);
      }}
    >
      {props.summary}
      <Show when={open()}>{props.children}</Show>
    </details>
  );
}
