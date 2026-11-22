import { API_URL, AUTHELIA_URL } from "$app/env/public";

import { type RequestHandler } from "@sveltejs/kit";

export const prerender = true;

export const GET: RequestHandler = () => {
    return Response.json({
        api: API_URL,
        authelia: AUTHELIA_URL,
    });
};
