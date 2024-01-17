import { A } from "@solidjs/router";
import { JSX } from "solid-js";

type ButtonProps = {
  as: "button";
} & JSX.ButtonHTMLAttributes<HTMLButtonElement>;
type AnchorProps = {
  as: "a";
  href: string;
} & JSX.AnchorHTMLAttributes<HTMLAnchorElement>;

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
      const { as, ...rest } = props;
      return (
        <A
          {...rest}
          class="rounded bg-gray-100 p-2 shadow mx-auto text-zinc-500"
        >
          {props.children}
        </A>
      );
  }
}
