import { createSignal } from "solid-js";
import { t } from "../i18n";
import Sheet from "./Sheet";

/**
 * Asks for the note and materials of the segment that is being closed.
 *
 * Mount it conditionally (inside `<Show when={...} keyed>`) so the fields are
 * re-initialised for every transition.
 */
export default function NoteSheet(props: {
  open: boolean;
  title: string;
  initialNote?: string;
  initialMaterial?: string;
  saveLabel?: string;
  allowSkip?: boolean;
  onClose: () => void;
  onSave: (note: string, material: string) => void;
  onSkip?: () => void;
}) {
  const [note, setNote] = createSignal(props.initialNote ?? "");
  const [material, setMaterial] = createSignal(props.initialMaterial ?? "");

  return (
    <Sheet open={props.open} title={props.title} onClose={props.onClose}>
      <div class="space-y-3">
        <div>
          <label class="label" for="sheet-note">
            {t("note.note")}
          </label>
          <textarea
            id="sheet-note"
            rows={3}
            class="field resize-none"
            placeholder={t("note.notePlaceholder")}
            value={note()}
            onInput={(e) => setNote(e.currentTarget.value)}
          />
        </div>
        <div>
          <label class="label" for="sheet-material">
            {t("note.material")}
          </label>
          <textarea
            id="sheet-material"
            rows={2}
            class="field resize-none"
            placeholder={t("note.materialPlaceholder")}
            value={material()}
            onInput={(e) => setMaterial(e.currentTarget.value)}
          />
        </div>
        <div class="flex gap-2 pt-1">
          <button
            type="button"
            class="btn-ghost flex-1"
            onClick={() => (props.allowSkip && props.onSkip ? props.onSkip() : props.onClose())}
          >
            {props.allowSkip ? t("note.skip") : t("common.close")}
          </button>
          <button
            type="button"
            class="btn-primary flex-1"
            onClick={() => props.onSave(note().trim(), material().trim())}
          >
            {props.saveLabel ?? t("note.save")}
          </button>
        </div>
      </div>
    </Sheet>
  );
}
