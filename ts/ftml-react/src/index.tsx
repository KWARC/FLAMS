import {FTML, ftmlSetup,renderDocument,renderFragment,FTMLConfig as FTMLConfigI,initialize as init} from "@kwarc/ftml-viewer";
import * as Flams from "@kwarc/flams";

import React, { ReactNode, useContext, useEffect, useRef } from "react";
import { FTMLContext, useLeptosTunnel, useLeptosTunnels } from "./leptos";

export const initialize = init;

export function getFlamsServer(): Flams.FLAMSServer {
  return new Flams.FLAMSServer(FTML.get_server_url());
}

export {FTML} from "@kwarc/ftml-viewer";
export * as Flams from "@kwarc/flams";


/**
 * Configurables for FTML rendering.
 * Every attribute is inherited from ancestor nodes *unless explicitly overridden*.
 */
export interface FTMLConfig {

  /**
   * whether to allow hovers
   */
  allowHovers?: boolean,
  
  /** may return a react component to *insert* after the title of a section
   * @param uri the uri of the section
   * @param lvl the level of the section
   * @return a react component to insert
   */
  onSectionTitle?: (
    uri: FTML.DocumentElementURI,
    lvl: FTML.SectionLevel,
  ) => ReactNode | undefined;

  /**
   * may return a react component to wrap around a fragment (e.g. Section, Definition, Problem, etc.)
   * @param uri the uri of the fragment
   * @param kind the fragment's kind
   * @return a react component to wrap around its argument
   */
  onFragment?: (
    uri: FTML.DocumentElementURI,
    kind: FTML.FragmentKind,
  ) => ((ch: ReactNode) => ReactNode) | undefined;

  problemStates?: FTML.ProblemStates | undefined;
  onProblem?: ((response: FTML.ProblemResponse) => void) | undefined;
}

/**
 * See {@link FTMLConfig}
 */
export interface FTMLSetupArgs extends FTMLConfig {
  children: ReactNode;
}

/**
 * Sets up Leptos' reactive system
 */
export const FTMLSetup: React.FC<FTMLSetupArgs> = (args) => {
  const mountRef = useRef<HTMLDivElement>(null);
  const main = useLeptosTunnel();
  const { addTunnel, TunnelRenderer } = useLeptosTunnels();

  useEffect(() => {
    if (!mountRef.current) return;
    const handle = ftmlSetup(
      mountRef.current,
      (e, o) => {
        main.addTunnel(
          e,
          <>
            {args.children}
            <TunnelRenderer />
          </>,
          o,
        );
      },
      toConfig(args, addTunnel),
    );
    return () => {
      handle.unmount();
    };
  }, []);

  return (
    <>
      <div ref={mountRef} style={{ display: "contents" }} />
      <main.TunnelRenderer />
    </>
  );
};

/**
 * See {@link FTMLConfig} and {@link FTML.DocumentOptions}
 */
export interface FTMLDocumentArgs extends FTMLConfig {
  document: FTML.DocumentOptions;
}

/**
 * render an FTML document
 */
export const FTMLDocument: React.FC<FTMLDocumentArgs> = (args) => {
  const mountRef = useRef<HTMLDivElement>(null);
  const { addTunnel, TunnelRenderer } = useLeptosTunnels();
  const context = useContext(FTMLContext);

  useEffect(() => {
    if (mountRef.current === null) return;
    const cont = context ? context.wasm_clone() : context;
    const handle = renderDocument(
      mountRef.current,
      args.document,
      cont,
      toConfig(args, addTunnel),
    );
    return () => {
      handle.unmount();
    };
  }, []);
  return (
    <div style={{ textAlign: "start" }}>
      <div ref={mountRef} />
      <TunnelRenderer />
    </div>
  );
};

/**
 * See {@link FTMLConfig} and {@link FTML.FragmentOptions}
 */
export interface FTMLFragmentArgs extends FTMLConfig {
  fragment: FTML.FragmentOptions;
}

/**
 * render an FTML fragment
 */
export const FTMLFragment: React.FC<FTMLFragmentArgs> = (args) => {
  const mountRef = useRef<HTMLDivElement>(null);
  const { addTunnel, TunnelRenderer } = useLeptosTunnels();
  const context = useContext(FTMLContext);

  useEffect(() => {
    if (!mountRef.current) return;
    const cont = context ? context.wasm_clone() : context;
    const handle = renderFragment(
      mountRef.current,
      args.fragment,
      cont,
      toConfig(args, addTunnel),
    );
    return () => {
      handle.unmount();
    };
  }, []);
  return (
    <div style={{ textAlign: "start" }}>
      <div ref={mountRef} />
      <TunnelRenderer />
    </div>
  );
};

const ElemToReact: React.FC<{
  elems: ChildNode[];
  ctx: FTML.LeptosContext;
}> = ({ elems, ctx }) => {
  const ref = useRef<HTMLDivElement>(null);
  const done = useRef<boolean>(false);
  useEffect(() => {
    if (ref.current && !done.current) {
      done.current = true;
      //console.log("Mounting",uri);
      ref.current.replaceChildren(...elems);
    }
  }, []);
  return (
    <FTMLContext.Provider value={ctx}>
      <div ref={ref} style={{ display: "contents" }} />
    </FTMLContext.Provider>
  );
};

function elemToReact(
  uri: FTML.DocumentElementURI,
  elem: HTMLDivElement,
  ctx: FTML.LeptosContext,
): ReactNode {
  //console.log("Doing",uri);
  const chs = Array.from(elem.childNodes);
  chs.forEach((c) => elem.removeChild(c));
  return <ElemToReact elems={chs} ctx={ctx} />;
}

function toConfig(
  config: FTMLConfig,
  addTunnel: (
    element: Element,
    node: ReactNode,
    context: FTML.LeptosContext,
  ) => string,
): FTMLConfigI {
  const otO = config.onSectionTitle;
  const onSectionTitle = otO
    ? (uri: FTML.DocumentElementURI, lvl: FTML.SectionLevel) => {
        const r = otO(uri, lvl);
        return r
          ? (elem: HTMLDivElement, ctx: FTML.LeptosContext) => {
              addTunnel(elem, r, ctx);
            }
          : undefined;
      }
    : undefined;

  const ofO = config.onFragment;
  const onFragment = ofO
    ? (uri: FTML.DocumentElementURI, kind: FTML.FragmentKind) => {
        const r = ofO(uri, kind);
        return r
          ? (elem: HTMLDivElement, ctx: FTML.LeptosContext) => {
              const ret = r(elemToReact(uri, elem, ctx));
              return addTunnel(elem, ret, ctx);
            }
          : undefined;
      }
    : undefined;

  return {
    onSectionTitle: onSectionTitle,
    onFragment: onFragment,
    problemStates: config.problemStates,
    onProblem: config.onProblem,
    allowHovers: config.allowHovers,
  };
}
