import * as React from "React";

type Task = {
    id: string;
    title: string;
    done: boolean;
    priority: "low" | "normal" | "high";
};

type Filter = "all" | "active" | "done";

type BadgeTone = "neutral" | "positive" | "warning" | "critical";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

type BadgeProps = {
    label: string;
    tone: BadgeTone;
};

type ButtonProps = {
    label: string;
    variant: ButtonVariant;
    onClick: () => void;
};

type TextInputProps = {
    value: string;
    placeholder: string;
    onChange: (value: string) => void;
};

type TaskRowProps = {
    task: Task;
    onToggle: (id: string) => void;
};

type TaskListProps = {
    tasks: Array<Task>;
    onToggle: (id: string) => void;
};

type SummaryProps = {
    total: number;
    remaining: number;
};

type HeaderProps = {
    title: string;
    subtitle: string;
};

type EmptyStateProps = {
    message: string;
    onReset: () => void;
};

type ToolbarProps = {
    filter: Filter;
    onFilterChange: (filter: Filter) => void;
};

type FooterProps = {
    note: string;
};

type TaskDetailProps = {
    task: Task | null;
    onClose: () => void;
    onToggle: (id: string) => void;
};

const filters: Array<Filter> = ["all", "active", "done"];

const priorityLabels: Record<Task["priority"], string> = {
    low: "low",
    normal: "normal",
    high: "high",
};

const toneForPriority = (priority: Task["priority"]): BadgeTone => {
    if (priority === "high") return "critical";
    if (priority === "normal") return "neutral";
    return "warning";
};

const Badge = ({ label, tone }: BadgeProps): React.JSX.Element => {
    const className: string = `badge badge-${tone}`;
    return <span className={className}>{label}</span>;
};

const Button = ({
    label,
    variant,
    onClick,
}: ButtonProps): React.JSX.Element => {
    const className: string = `btn btn-${variant}`;

    return (
        <button
            type="button"
            className={className}
            onClick={onClick}
        >
            {label}
        </button>
    );
};

const TextInput = ({
    value,
    placeholder,
    onChange,
}: TextInputProps): React.JSX.Element => {
    const handleChange = (next: string): void => {
        onChange(next);
    };

    return (
        <input
            type="text"
            className="text-input"
            value={value}
            placeholder={placeholder}
            onChange={(event) => handleChange(event.target.value)}
        />
    );
};

const TaskRow = ({ task, onToggle }: TaskRowProps): React.JSX.Element => {
    const handleToggle = (): void => {
        onToggle(task.id);
    };

    const statusLabel: string = task.done ? "done" : "open";

    return (
        <li className="task-row">
            <label className="task-row-check">
                <input
                    type="checkbox"
                    checked={task.done}
                    onChange={handleToggle}
                />
                <span className={task.done ? "task-title done" : "task-title"}>
                    {task.title}
                </span>
            </label>
            <Badge
                label={priorityLabels[task.priority]}
                tone={toneForPriority(task.priority)}
            />
            <Badge
                label={statusLabel}
                tone={task.done ? "positive" : "neutral"}
            />
            <Button
                label={task.done ? "Reopen" : "Complete"}
                variant="ghost"
                onClick={handleToggle}
            />
        </li>
    );
};

const TaskList = ({ tasks, onToggle }: TaskListProps): React.JSX.Element => {
    const rows = tasks.map((task): React.JSX.Element => (
        <TaskRow
            key={task.id}
            task={task}
            onToggle={onToggle}
        />
    ));

    return (
        <section className="task-list">
            <ul className="task-list-rows">{rows}</ul>
        </section>
    );
};

const Summary = ({ total, remaining }: SummaryProps): React.JSX.Element => {
    const message = React.useMemo((): string => {
        const prefix: string = remaining === 0 ? "All caught up" : "Work left";
        return `${prefix}: ${remaining} of ${total}`;
    }, [total, remaining]);

    return (
        <div className="summary">
            <span className="summary-text">{message}</span>
            <Badge
                label={`${total} total`}
                tone="neutral"
            />
            <Badge
                label={`${remaining} remaining`}
                tone={remaining > 0 ? "warning" : "positive"}
            />
        </div>
    );
};

const Header = ({ title, subtitle }: HeaderProps): React.JSX.Element => {
    return (
        <header className="page-header">
            <div className="page-header-text">
                <h1 className="page-title">{title}</h1>
                <p className="page-subtitle">{subtitle}</p>
            </div>
            <div className="page-header-actions">
                <Button
                    label="New task"
                    variant="primary"
                    onClick={() => void 0}
                />
            </div>
        </header>
    );
};

const EmptyState = ({
    message,
    onReset,
}: EmptyStateProps): React.JSX.Element => {
    return (
        <div className="empty-state">
            <p className="empty-state-message">{message}</p>
            <Button
                label="Reset tasks"
                variant="secondary"
                onClick={onReset}
            />
        </div>
    );
};

const Toolbar = ({
    filter,
    onFilterChange,
}: ToolbarProps): React.JSX.Element => {
    const buttons = filters.map((candidate): React.JSX.Element => (
        <Button
            key={candidate}
            label={candidate}
            variant={candidate === filter ? "primary" : "secondary"}
            onClick={() => onFilterChange(candidate)}
        />
    ));

    return (
        <div className="toolbar">
            <div className="toolbar-segment">{buttons}</div>
        </div>
    );
};

const StatsRing = ({ total, remaining }: SummaryProps): React.JSX.Element => {
    const completedCount: number = total - remaining;

    const pct: number =
        total === 0 ? 0 : Math.round((completedCount / total) * 100);

    return (
        <div className="stats-ring">
            <svg
                className="stats-ring-svg"
                viewBox="0 0 36 36"
            >
                <path
                    className="stats-ring-bg"
                    d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
                />
                <path
                    className="stats-ring-value"
                    strokeDasharray={`${pct}, 100`}
                    d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
                />
            </svg>
            <span className="stats-ring-label">{pct}% done</span>
            <Badge
                label={`${completedCount}/${total}`}
                tone="neutral"
            />
        </div>
    );
};

const TaskDetail = ({
    task,
    onClose,
    onToggle,
}: TaskDetailProps): React.JSX.Element => {
    const handleToggle = (): void => {
        if (task === null) return void 0;
        onToggle(task.id);
    };

    if (task === null) {
        return (
            <aside className="task-detail empty">
                <span className="task-detail-empty-text">
                    No task selected.
                </span>
                <Button
                    label="Close panel"
                    variant="ghost"
                    onClick={onClose}
                />
            </aside>
        );
    }

    return (
        <aside className="task-detail">
            <div className="task-detail-head">
                <h3 className="task-detail-title">{task.title}</h3>
                <Button
                    label="Close"
                    variant="ghost"
                    onClick={onClose}
                />
            </div>
            <div className="task-detail-body">
                <Badge
                    label={priorityLabels[task.priority]}
                    tone={toneForPriority(task.priority)}
                />
                <Badge
                    label={task.done ? "done" : "open"}
                    tone={task.done ? "positive" : "neutral"}
                />
                <p className="task-detail-id">id: {task.id}</p>
            </div>
            <div className="task-detail-actions">
                <Button
                    label={task.done ? "Mark as open" : "Mark as done"}
                    variant={task.done ? "secondary" : "primary"}
                    onClick={handleToggle}
                />
            </div>
        </aside>
    );
};

const Footer = ({ note }: FooterProps): React.JSX.Element => {
    return (
        <footer className="page-footer">
            <span className="page-footer-note">{note}</span>
            <Button
                label="Dismiss"
                variant="ghost"
                onClick={() => void 0}
            />
        </footer>
    );
};

const seedTasks: Array<Task> = [
    { id: "t-1", title: "Read the changelog", done: false, priority: "low" },
    { id: "t-2", title: "Write release notes", done: true, priority: "normal" },
    { id: "t-3", title: "Ship the compiler", done: false, priority: "high" },
    {
        id: "t-4",
        title: "Review the benchmarks",
        done: false,
        priority: "normal",
    },
    { id: "t-5", title: "Update the docs", done: true, priority: "low" },
    { id: "t-6", title: "Tag the release", done: false, priority: "high" },
    { id: "t-7", title: "Sync with the team", done: false, priority: "normal" },
    { id: "t-8", title: "Archive old issues", done: true, priority: "low" },
];

const Page = (): React.JSX.Element => {
    const [tasks, setTasks] = React.useState<Array<Task>>(seedTasks);
    const [filter, setFilter] = React.useState<Filter>("all");
    const [draft, setDraft] = React.useState<string>("");
    const [selectedId, setSelectedId] = React.useState<string | null>(null);

    const handleToggle = React.useCallback(
        (id: string): void => {
            setTasks((current) =>
                current.map((task): Task => {
                    if (task.id === id) return { ...task, done: !task.done };
                    return task;
                }),
            );
        },
        [setTasks],
    );

    const handleFilterChange = React.useCallback(
        (next: Filter): void => {
            setFilter(next);
        },
        [setFilter],
    );

    const handleDraftChange = React.useCallback(
        (next: string): void => {
            setDraft(next);
        },
        [setDraft],
    );

    const handleAdd = React.useCallback((): void => {
        const title: string = draft.trim();

        if (title.length === 0) return void 0;

        const nextTask: Task = {
            id: `t-${tasks.length + 1}-${title.length}`,
            title,
            done: false,
            priority: "normal",
        };

        setTasks([...tasks, nextTask]);

        setDraft("");
    }, [draft, tasks, setTasks]);

    const handleReset = React.useCallback((): void => {
        setTasks(seedTasks);
        setFilter("all");
        setDraft("");
        setSelectedId(null);
    }, [setTasks, setFilter]);

    const handleCloseDetail = React.useCallback((): void => {
        setSelectedId(null);
    }, [setSelectedId]);

    const selectedTask = React.useMemo((): Task | null => {
        if (selectedId === null) return null;
        return tasks.find((task) => task.id === selectedId) ?? null;
    }, [tasks, selectedId]);

    const handleClearCompleted = React.useCallback((): void => {
        setTasks((current) => current.filter((task) => !task.done));
        setSelectedId(null);
    }, [setTasks]);

    const completedCount = React.useMemo((): number => {
        return tasks.filter((task) => task.done).length;
    }, [tasks]);

    const visibleTasks = React.useMemo((): Array<Task> => {
        if (filter === "active") return tasks.filter((task) => !task.done);
        if (filter === "done") return tasks.filter((task) => task.done);
        return tasks;
    }, [tasks, filter]);

    const remaining = React.useMemo((): number => {
        return tasks.filter((task) => !task.done).length;
    }, [tasks]);

    const emptyMessage: string =
        filter === "done" ? "Nothing completed yet." : "No active tasks.";

    return (
        <div className="page">
            <Header
                title="Telarel Tasks"
                subtitle="A demo page rendered by the benchmark fixture"
            />
            <Toolbar
                filter={filter}
                onFilterChange={handleFilterChange}
            />
            <div className="page-body">
                <div className="page-input">
                    <TextInput
                        value={draft}
                        placeholder="Add a task"
                        onChange={handleDraftChange}
                    />
                    <Button
                        label="Add"
                        variant="primary"
                        onClick={handleAdd}
                    />
                </div>
                {visibleTasks.length > 0 ? (
                    <TaskList
                        tasks={visibleTasks}
                        onToggle={handleToggle}
                    />
                ) : (
                    <EmptyState
                        message={emptyMessage}
                        onReset={handleReset}
                    />
                )}
                {selectedTask !== null && (
                    <TaskDetail
                        task={selectedTask}
                        onClose={handleCloseDetail}
                        onToggle={handleToggle}
                    />
                )}
                {remaining === 0 && tasks.length > 0 && (
                    <div className="page-banner">
                        <span>All tasks are complete.</span>
                        <Button
                            label="Start over"
                            variant="danger"
                            onClick={handleReset}
                        />
                    </div>
                )}
                {completedCount > 0 && remaining > 0 && (
                    <div className="page-clear">
                        <span>{completedCount} completed.</span>
                        <Button
                            label="Clear completed"
                            variant="secondary"
                            onClick={handleClearCompleted}
                        />
                    </div>
                )}
            </div>
            <Summary
                total={tasks.length}
                remaining={remaining}
            />
            <StatsRing
                total={tasks.length}
                remaining={remaining}
            />
            <Footer note="Built with the Telarel benchmark suite" />
        </div>
    );
};

export default Page;
