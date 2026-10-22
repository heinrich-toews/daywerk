import { Match, Switch } from "solid-js";
import { openProjectId, openTaskId, taskView } from "../../state";
import ProjectDetail from "./ProjectDetail";
import ProjectsView from "./ProjectsView";
import TaskDetail from "./TaskDetail";
import TaskListView from "./TaskListView";
import TaskSearch from "./TaskSearch";
import TaskStats from "./TaskStats";

export default function TaskWorld() {
  return (
    <Switch>
      <Match when={openTaskId() !== null}>
        <TaskDetail />
      </Match>
      <Match when={openProjectId() !== null}>
        <ProjectDetail />
      </Match>
      <Match when={taskView() === "projects"}>
        <ProjectsView />
      </Match>
      <Match when={taskView() === "tasks"}>
        <TaskListView />
      </Match>
      <Match when={taskView() === "search"}>
        <TaskSearch />
      </Match>
      <Match when={taskView() === "stats"}>
        <TaskStats />
      </Match>
    </Switch>
  );
}
