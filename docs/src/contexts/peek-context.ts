"use client";

import * as React from "react";

/**
 * Whether the surrounding `PeekCode` opts its code blocks into peeking.
 */
const PeekContext = React.createContext<boolean>(false);

export { PeekContext };
