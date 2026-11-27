import type { DocMacroOptions } from "fumadocs-mdx/macro";

import { lucideIconsPlugin } from "fumadocs-core/source/plugins/lucide-icons";
import { defineDocs } from "fumadocs-mdx/macro";
import { HomeLayout } from "fumadocs-ui/layouts/home";
import { createPress } from "fumapress";
import { linkValidationPlugin } from "fumapress/plugins/link-validation";

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

export const app = createPress(docs.toFumadocsSource(), {
    mode: "force-static",
    loaderPlugins: [
        // lucide icons
        lucideIconsPlugin(),
    ],
}).plugins([
    // link validation
    linkValidationPlugin(),
]);

export default app.config({
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
    ui: {
        layoutProps: {
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
        renderPage(props) {
            // The Home layout is not a `renderDefault()` option,
            // and `/` must stay a content page.
            if (props.page.path === "index.mdx") {
                const {
                    nav,
                    links,
                    githubUrl,
                    i18n,
                    themeSwitch,
                    searchToggle,
                } = props.layoutProps;

                return (
                    <HomeLayout
                        {...{
                            nav,
                            links,
                            githubUrl,
                            i18n,
                            themeSwitch,
                            searchToggle,
                        }}
                    >
                        <div
                            data-fd-home-layout-container
                            className={
                                "mx-auto flex w-full max-w-[1400px] flex-1 flex-col px-4 py-6"
                            }
                        >
                            {props.body}
                            <SiteFooter
                                className={
                                    "mt-8 text-center text-sm text-fd-muted-foreground"
                                }
                            />
                        </div>
                    </HomeLayout>
                );
            }

            return props.renderDefault({
                layout: "notebook",
                pageProps: {
                    footer: {
                        children: (
                            <SiteFooter
                                className={
                                    "mt-6 text-center text-sm text-fd-muted-foreground"
                                }
                            />
                        ),
                    },
                },
            });
        },
    },
});
