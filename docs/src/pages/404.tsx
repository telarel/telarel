import { DefaultNotFound } from "fumadocs-ui/layouts/home/not-found";

import { SiteFooter } from "#/components/site-footer";

const NotFound = (): React.JSX.Element => {
    return (
        <>
            <DefaultNotFound />
            <SiteFooter
                className={"py-6 text-center text-sm text-fd-muted-foreground"}
            />
        </>
    );
};

export default NotFound;
