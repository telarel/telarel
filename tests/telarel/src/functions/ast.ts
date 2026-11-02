type ProgramDeclaration = {
    id: { type: string; name: string };
};

type ProgramFixture = {
    body: Array<{
        declarations: Array<ProgramDeclaration>;
    }>;
};

const rootDeclaration = (program: ProgramFixture): ProgramDeclaration => {
    const statement: ProgramFixture["body"][number] | undefined =
        program.body[0];

    if (!statement) {
        throw new Error("expected a top-level statement");
    }

    const declaration: ProgramDeclaration | undefined = Array.isArray(
        statement.declarations,
    )
        ? statement.declarations[0]
        : void 0;

    if (declaration?.id.type !== "Identifier") {
        throw new Error("expected an identifier declaration");
    }

    return declaration;
};

const renameRootIdentifier = (options: {
    program: ProgramFixture;
    name: string;
}): void => {
    rootDeclaration(options.program).id.name = options.name;
};

const readRootIdentifier = (program: ProgramFixture): string =>
    rootDeclaration(program).id.name;

export type { ProgramFixture };
export { readRootIdentifier, renameRootIdentifier };
