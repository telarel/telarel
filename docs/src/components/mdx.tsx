import type { MermaidProps } from "fumadocs-mermaid/ui";
import type { MDXComponents } from "mdx/types";

import { Mermaid as FumaMermaid } from "fumadocs-mermaid/ui";
import defaultMdxComponents from "fumadocs-ui/mdx";

import { PeekCode, PeekCodeBlock } from "#/components/peek-code";

const Mermaid = (props: MermaidProps): React.JSX.Element => {
    return (
        <div className={"mermaid"}>
            <FumaMermaid {...props} />
        </div>
    );
};

export default {
    ...defaultMdxComponents,
    Mermaid,
    PeekCode,
    pre: PeekCodeBlock,
} satisfies MDXComponents;
