import type { ObjectHook, PluginOrder } from "#/@types/plugin";

/**
 * Per-hook ordering metadata, the JS-thread shape of a normalized hook.
 */
type ObjectHookMeta = {
    order?: PluginOrder;
};

/**
 * A hook split into its callable handler and its ordering metadata.
 */
type NormalizedHook<T> = {
    handler: T | void;
    meta: ObjectHookMeta;
};

/**
 * Normalize a plain function or an `{ order?, handler }` object into a handler
 * plus its ordering metadata. A missing hook normalizes to an absent handler
 * with empty metadata.
 */
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
