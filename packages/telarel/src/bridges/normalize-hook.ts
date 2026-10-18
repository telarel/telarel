import type { ObjectHook, PluginOrder } from "#/@types/plugin";

/**
 * Per-hook ordering metadata, the JS-thread shape of a normalized hook.
 */
type ObjectHookMeta = {
    order?: PluginOrder;
};

type NormalizedHook<T> = {
    handler: T | void;
    meta: ObjectHookMeta;
};

const normalizeHook = <T>(hook: ObjectHook<T> | void): NormalizedHook<T> => {
    if (hook === void 0) {
        return { handler: void 0, meta: {} };
    }

    if (typeof hook === "function") {
        return { handler: hook, meta: {} };
    }

    const { handler, order } = hook as { order?: PluginOrder; handler: T };

    return { handler, meta: { order } };
};

export type { NormalizedHook, ObjectHookMeta };
export { normalizeHook };
