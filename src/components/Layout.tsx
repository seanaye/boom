export default function Layout(props: { children: JSX.Element }) {
  return (
    <div class="rounded-xl p-4 bg-amber-100 text-zinc-600">
      {props.children}
    </div>
  );
}
