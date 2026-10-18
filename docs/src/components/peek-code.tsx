"use client";

import { CodeBlock, Pre } from "fumadocs-ui/components/codeblock";
import { ChevronDown } from "lucide-react";
import * as React from "react";

import { PeekContext } from "#/contexts/peek-context";

const PEEK_CLASS: string = "peek-code";
const DETECTED_CLASS: string = "peek-detected";
const EXPANDED_CLASS: string = "peek-expanded";

const BAR_CLASS: string =
    "absolute inset-x-0 bottom-0 z-2 flex h-11 cursor-pointer items-center justify-center gap-1.5 rounded-b-xl border-t border-fd-border bg-fd-card/85 text-sm font-medium text-fd-muted-foreground backdrop-blur-sm transition-colors hover:bg-fd-accent/60 hover:text-fd-accent-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-fd-ring focus-visible:outline-none";
const CHEVRON_CLASS: string = "size-4 shrink-0";

const countLines = (region: HTMLElement): number => {
    const text: string = region.querySelector("pre")?.textContent ?? "";

    return text === "" ? 0 : text.replace(/\n$/, "").split("\n").length;
};

type PeekBarProps = {
    lines: number;
    onExpand: () => void;
};

const PeekBar = ({ lines, onExpand }: PeekBarProps): React.JSX.Element => (
    <button
        type={"button"}
        aria-expanded={false}
        onClick={onExpand}
        className={BAR_CLASS}
    >
        {`Show all ${lines} lines`}
        <ChevronDown className={CHEVRON_CLASS} />
    </button>
);

type CodeProps = React.HTMLAttributes<HTMLPreElement> & {
    icon?: React.ReactNode;
};

type ActionProps = {
    className?: string;
    children?: React.ReactNode;
};

const PeekedCodeBlock = ({
    children,
    ...props
}: CodeProps): React.JSX.Element => {
    const figureRef: React.RefObject<HTMLElement | null> = React.useRef(null);
    const [lineCount, setLineCount] = React.useState<number | null>(null);
    const [isExpanded, setIsExpanded] = React.useState<boolean>(false);

    React.useLayoutEffect((): void => {
        const region: HTMLElement | null =
            figureRef.current?.querySelector('[role="region"]') ?? null;

        if (region === null || region.scrollHeight <= region.clientHeight) {
            return;
        }

        setLineCount(countLines(region));
    }, []);

    const className: string = [
        props.className,
        PEEK_CLASS,
        lineCount === null ? "" : DETECTED_CLASS,
        isExpanded ? EXPANDED_CLASS : "",
    ]
        .filter(Boolean)
        .join(" ");

    return (
        <CodeBlock
            {...props}
            ref={figureRef}
            className={className}
            Actions={({
                className: actionClassName,
                children: actionChildren,
            }: ActionProps) => (
                <>
                    <div className={actionClassName}>{actionChildren}</div>
                    {lineCount !== null && !isExpanded && (
                        <PeekBar
                            lines={lineCount}
                            onExpand={() => setIsExpanded(true)}
                        />
                    )}
                </>
            )}
        >
            <Pre>{children}</Pre>
        </CodeBlock>
    );
};

type PeekCodeProps = {
    children: React.ReactNode;
};

/**
 * Opts the code blocks it wraps into the peek behavior.
 */
const PeekCode = ({ children }: PeekCodeProps): React.JSX.Element => (
    <PeekContext value>{children}</PeekContext>
);

/**
 * Renders a code block, capping and expanding it when it overflows.
 */
const PeekCodeBlock = (props: CodeProps): React.JSX.Element => {
    const isPeeked: boolean = React.use(PeekContext);

    if (!isPeeked) {
        const { children, ...rest } = props;

        return (
            <CodeBlock {...rest}>
                <Pre>{children}</Pre>
            </CodeBlock>
        );
    }

    return <PeekedCodeBlock {...props} />;
};

export { PeekCode, PeekCodeBlock };
