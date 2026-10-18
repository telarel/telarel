"use client";

const currentYear: number = new Date().getFullYear();

type SiteFooterProps = {
    className?: string;
};

/**
 * Renders the copyright footer.
 */
const SiteFooter = ({ className }: SiteFooterProps): React.JSX.Element => (
    <p className={className}>{`Copyright © ${currentYear}, Alpheus`}</p>
);

export { SiteFooter };
