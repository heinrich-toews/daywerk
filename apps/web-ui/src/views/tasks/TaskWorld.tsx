import { Match, Switch } from "solid-js";
import { t } from "../../i18n";
import { taskView } from "../../state";

function Placeholder(props: { title: string; hint: string }) {
  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">{props.title}</h1>
      <p class="card p-6 text-center text-sm text-muted">{props.hint}</p>
    </div>
  );
}

export default function TaskWorld() {
  return (
    <Switch>
      <Match when={taskView() === "projects"}>
        <Placeholder title={t("nav.projects")} hint={t("tasks.placeholder")} />
      </Match>
      <Match when={taskView() === "tasks"}>
        <Placeholder title={t("nav.tasks")} hint={t("tasks.placeholder")} />
      </Match>
      <Match when={taskView() === "search"}>
        <Placeholder title={t("nav.search")} hint={t("tasks.placeholder")} />
      </Match>
      <Match when={taskView() === "stats"}>
        <Placeholder title={t("nav.stats")} hint={t("tasks.placeholder")} />
      </Match>
    </Switch>
  );
}
