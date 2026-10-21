import { Show, type JSX } from "solid-js";

/** Bottom sheet on phones, centered dialog on larger screens. */
export default function Sheet(props: {
  open: boolean;
  title: string;
  onClose: () => void;
  children: JSX.Element;
}) {
  return (
    <Show when={props.open}>
      <div
        class="fixed inset-0 z-50 flex items-end justify-center bg-black/40 p-0 sm:items-center sm:p-4"
        onClick={props.onClose}
      >
        <div
          class="w-full max-w-lg rounded-t-2xl bg-card p-4 shadow-xl sm:rounded-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          <div class="mb-3 flex items-center justify-between gap-3">
            <h2 class="min-w-0 truncate font-display text-lg font-bold uppercase tracking-wide">
              {props.title}
            </h2>
            <button
              type="button"
              onClick={props.onClose}
              class="rounded-lg px-2 py-1 text-lg leading-none text-muted hover:bg-soft hover:text-foreground"
              aria-label="close"
            >
              ✕
            </button>
          </div>
          {props.children}
        </div>
      </div>
    </Show>
  );
}
