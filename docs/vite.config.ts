import tailwindcss from "@tailwindcss/vite";
import { fumadocsMdx } from "fumadocs-mdx/vite";
import { remarkMdxMermaid } from "fumadocs-mermaid";
import press from "fumapress/vite";
import { defineConfig } from "vite";

export default defineConfig({
    resolve: {
        tsconfigPaths: true,
    },
    plugins: [
        press(),
        fumadocsMdx({
            globalOptions: {
                mdxOptions: {
                    remarkPlugins: [remarkMdxMermaid],
                    rehypeCodeOptions: {
                        themes: {
                            light: "light-plus",
                            dark: "dark-plus",
                        },
                    },
                },
            },
        }),
        tailwindcss(),
    ],
    optimizeDeps: {
        include: ["mermaid"],
    },
    server: {
        port: 3001,
    },
    preview: {
        port: 3000,
    },
});
