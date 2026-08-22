import tailwind from "@tailwindcss/vite";

export default defineNuxtConfig({
    app: {
        head: {
            title: "Angad Auth",
            htmlAttrs: {
                "data-carbon-theme": "g90",
            },
            link: [{ rel: "icon", type: "image/png", href: "/favicon.png" }],
            meta: [
                {
                    name: "viewport",
                    content: "width=device-width, initial-scale=1",
                },
            ],
        },
    },
    components: [{ path: "~/components", pathPrefix: false }],
    nitro: {
        routeRules: {
            "/**": {
                headers: {
                    "Cross-Origin-Embedder-Policy": "require-corp",
                    "Cross-Origin-Opener-Policy": "same-origin",
                },
            },
        },
    },
    devtools: { enabled: false },
    modules: ["@nuxt/icon", "@vueuse/nuxt"],
    pages: true,
    css: ["~/style/carbon.scss", "~/style/main.css"],
    postcss: {
        plugins: {
            autoprefixer: {},
        },
    },
    vite: {
        plugins: [
            tailwind(),
            {
                name: "headers",
                configureServer(server) {
                    server.middlewares.use((req, res, next) => {
                        res.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
                        res.setHeader("Cross-Origin-Opener-Policy", "same-origin");
                        next();
                    });
                },
            },
        ],
        worker: {
            format: "es",
        },
        build: {
            rollupOptions: {
                external: ["/wasm/auth_crypto.js"],
            },
        },
    },
    icon: {
        serverBundle: {
            collections: ["hugeicons"],
        },
    },
    ssr: false,
    compatibilityDate: "2025-08-18",
});
