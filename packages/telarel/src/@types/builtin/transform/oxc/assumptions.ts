/**
 * Compiler assumptions for producing smaller output.
 *
 * Mirrors oxc's `CompilerAssumptions` (camelCase, unknown fields rejected),
 * which the npm `oxc-transform` types describe only partially. Fields marked
 * "Not implemented by oxc." are accepted by the bridge but have no effect.
 */
type OxcCompilerAssumptions = {
    /**
     * Assume array-like values are iterable. Not implemented by oxc.
     */
    arrayLikeIsIterable?: boolean;
    /**
     * Assume re-exported bindings are constant. Not implemented by oxc.
     */
    constantReexports?: boolean;
    /**
     * Assume `super` property writes do not require runtime checks. Not
     * implemented by oxc.
     */
    constantSuper?: boolean;
    /**
     * Treat `import.meta` properties as enumerable. Not implemented by oxc.
     */
    enumerableModuleMeta?: boolean;
    /**
     * Ignore `Function#length` when lowering function wrappers.
     */
    ignoreFunctionLength?: boolean;
    /**
     * Ignore the preferred hint passed to `Symbol.toPrimitive`. Not implemented
     * by oxc.
     */
    ignoreToPrimitiveHint?: boolean;
    /**
     * Assume iterable operations only receive arrays. Not implemented by oxc.
     */
    iterableIsArray?: boolean;
    /**
     * Emit mutable template objects. Not implemented by oxc.
     */
    mutableTemplateObject?: boolean;
    /**
     * Assume class constructors are never called without `new`. Not implemented
     * by oxc.
     */
    noClassCalls?: boolean;
    /**
     * Assume `document.all` is not special-cased.
     */
    noDocumentAll?: boolean;
    /**
     * Skip namespace import completeness checks. Not implemented by oxc.
     */
    noIncompleteNsImportDetection?: boolean;
    /**
     * Assume no new `this` capture is needed for arrows. Not implemented by
     * oxc.
     */
    noNewArrows?: boolean;
    /**
     * Assume private fields are always initialized before access. Not
     * implemented by oxc.
     */
    noUninitializedPrivateFieldAccess?: boolean;
    /**
     * Assume object rest does not need to copy symbol properties.
     */
    objectRestNoSymbols?: boolean;
    /**
     * Represent private fields as symbols. Not implemented by oxc.
     */
    privateFieldsAsSymbols?: boolean;
    /**
     * Represent private fields as string-keyed properties.
     */
    privateFieldsAsProperties?: boolean;
    /**
     * Assume property reads can be treated as pure.
     */
    pureGetters?: boolean;
    /**
     * Assume class methods can be assigned directly. Not implemented by oxc.
     */
    setClassMethods?: boolean;
    /**
     * Assume computed properties can be assigned directly. Not implemented by
     * oxc.
     */
    setComputedProperties?: boolean;
    /**
     * Assume public class fields do not shadow a getter in the class, its
     * subclasses or its superclass, so it is safe to assign instead of using
     * `Object.defineProperty`.
     */
    setPublicClassFields?: boolean;
    /**
     * Assume object spread can use direct assignment semantics. Not implemented
     * by oxc.
     */
    setSpreadProperties?: boolean;
    /**
     * Skip `for..of` iterator closing logic. Not implemented by oxc.
     */
    skipForOfIteratorClosing?: boolean;
    /**
     * Assume `super` can be invoked as a normal callable constructor. Not
     * implemented by oxc.
     */
    superIsCallableConstructor?: boolean;
};

export type { OxcCompilerAssumptions };
