import { JSX } from "solid-js";

type ButtonProps = {
  as: "button";
} & JSX.ButtonHTMLAttributes<HTMLButtonElement>;
type AnchorProps = { as: "a" } & JSX.AnchorHTMLAttributes<HTMLAnchorElement>;

type Props = ButtonProps | AnchorProps;

export function IconButton(props: Props) {
  switch (props.as) {
    case "button":
      return (
        <button
          {...props}
          class="rounded bg-gray-100 p-2 shadow mx-auto text-zinc-500"
        >
          {props.children}
        </button>
      );
    case "a":
      return (
        <a
          {...props}
          class="rounded bg-gray-100 p-2 shadow mx-auto text-zinc-500"
        >
          {props.children}
        </a>
      );
  }
}
