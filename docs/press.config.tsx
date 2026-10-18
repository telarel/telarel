import type { DocMacroOptions } from "fumadocs-mdx/macro";
import type { MermaidProps } from "fumadocs-mermaid/ui";

import { lucideIconsPlugin } from "fumadocs-core/source/plugins/lucide-icons";
import { defineDocs } from "fumadocs-mdx/macro";
import { Mermaid as FumaMermaid } from "fumadocs-mermaid/ui";
import { DefaultNotFound } from "fumadocs-ui/layouts/home/not-found";
import defaultMdxComponents, { createRelativeLink } from "fumadocs-ui/mdx";
import { defineConfig } from "fumapress";
import { fumadocsMdx } from "fumapress/adapters/mdx";
import { createHomeLayoutPage } from "fumapress/layouts/home";
import { createNotebookLayoutPage } from "fumapress/layouts/notebook";
import { linkValidationPlugin } from "fumapress/plugins/link-validation";

import { PeekCode, PeekCodeBlock } from "#/components/peek-code";
import { SiteFooter } from "#/components/site-footer";

const configDocs = {
    async: true,
    lastModified: true, // lastModified -> RSS dates + page footer
    postprocess: {
        includeProcessedMarkdown: true, // required by the llms.txt plugin
    },
} as const satisfies DocMacroOptions;

const docs = defineDocs({
    dir: "content",
    docs: configDocs,
});

const NotebookLayout = createNotebookLayoutPage<typeof config.$context>({
    renderPage: async ({ props, next }) =>
        await next({
            ...props,
            footer: {
                ...props.footer,
                children: (
                    <>
                        {props.footer?.children}
                        <SiteFooter
                            className={
                                "mt-6 text-center text-sm text-fd-muted-foreground"
                            }
                        />
                    </>
                ),
            },
        }),
});

const HomeLayout = createHomeLayoutPage<typeof config.$context>({
    render: async function (page) {
        const body = (await this.getPageBody(page))?.node;

        return {
            body: (
                <>
                    {body}
                    <SiteFooter
                        className={
                            "mt-8 text-center text-sm text-fd-muted-foreground"
                        }
                    />
                </>
            ),
        };
    },
});

const Mermaid = (props: MermaidProps): React.JSX.Element => {
    return (
        <div className={"mermaid"}>
            <FumaMermaid {...props} />
        </div>
    );
};

const config = defineConfig({
    mode: "static",
    site: {
        name: "Telarel",
        baseUrl: "https://telarel.github.io/",
        git: {
            provider: "github",
            user: "telarel",
            repo: "telarel",
            branch: "main",
        },
    },
    content: docs.toFumadocsSource(),
    defaultLayoutProps: {
        nav: {
            title: "Telarel",
        },
        links: [
            {
                text: "Guide",
                url: "/guide",
            },
            {
                text: "Plugins",
                url: "/plugins",
            },
            {
                text: "API",
                url: "/apis",
            },
        ],
    },
    loaderOptions: {
        plugins: [lucideIconsPlugin()],
    },
    renderPage: (props) =>
        props.page.path === "index.mdx" ? (
            <HomeLayout {...props} />
        ) : (
            <NotebookLayout {...props} />
        ),
    renderNotFound: () => (
        <>
            <DefaultNotFound />
            <SiteFooter
                className={"py-6 text-center text-sm text-fd-muted-foreground"}
            />
        </>
    ),
})
    .adapters(
        fumadocsMdx({
            async getMdxComponents(page) {
                return {
                    ...defaultMdxComponents,
                    a: createRelativeLink(await this.getLoader(), page),
                    Mermaid,
                    PeekCode,
                    pre: PeekCodeBlock,
                };
            },
        }),
    )
    .plugins(linkValidationPlugin());

export default config;
