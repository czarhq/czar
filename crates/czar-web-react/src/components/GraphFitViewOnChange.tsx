import { useEffect, useRef } from 'react';
import { useNodesInitialized, useReactFlow } from '@xyflow/react';

interface GraphFitViewOnChangeProps {
  trigger: string;
  isActive: boolean;
  fitOptions?: {
    padding?: number;
    minZoom?: number;
    maxZoom?: number;
  };
}

export default function GraphFitViewOnChange({ trigger, isActive, fitOptions }: GraphFitViewOnChangeProps) {
  const reactFlow = useReactFlow();
  const nodesInitialized = useNodesInitialized();
  const lastFittedTriggerRef = useRef<string | null>(null);

  useEffect(() => {
    if (!isActive || !nodesInitialized) return;
    if (lastFittedTriggerRef.current === trigger) return;
    lastFittedTriggerRef.current = trigger;

    const fitNow = () => {
      void reactFlow.fitView({ padding: 0.2, ...fitOptions, duration: 0 });
    };

    fitNow();
    const animationFrame = window.requestAnimationFrame(fitNow);
    const timeoutId = window.setTimeout(fitNow, 80);

    return () => {
      window.cancelAnimationFrame(animationFrame);
      window.clearTimeout(timeoutId);
    };
  }, [fitOptions, isActive, nodesInitialized, reactFlow, trigger]);

  return null;
}
