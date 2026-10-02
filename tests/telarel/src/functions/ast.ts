type ProgramFixture = {
    body: Array<{
        declarations: Array<{
            id: { type: string; name: string };
        }>;
    }>;
};

const renameRootIdentifier = (options: {
    program: ProgramFixture;
    name: string;
}): void => {
    const statement: ProgramFixture["body"][number] | undefined =
        options.program.body[0];

    if (!statement) {
        throw new Error("expected a top-level statement");
    }

    const declaration:
        | ProgramFixture["body"][number]["declarations"][number]
        | undefined = statement.declarations[0];

    if (declaration?.id.type !== "Identifier") {
        throw new Error("expected an identifier declaration");
    }

    declaration.id.name = options.name;
};

export type { ProgramFixture };
export { renameRootIdentifier };
