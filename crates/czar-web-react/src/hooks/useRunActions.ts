import { useCallback, useState } from 'react';
import { cancelRun, pauseRun, resumeRun } from '../api';

/** Cancel (with confirmation), pause and resume for runs, with their error state. */
export function useRunActions(refreshAll: () => void) {
  const [cancelRunIdToConfirm, setCancelRunIdToConfirm] = useState<string | null>(null);
  const [cancelRunSubmitting, setCancelRunSubmitting] = useState(false);
  const [cancelRunError, setCancelRunError] = useState<string | null>(null);
  const [runActionError, setRunActionError] = useState<string | null>(null);

  const handleCancelRun = useCallback(async (runId: string) => {
    setCancelRunError(null);
    setCancelRunIdToConfirm(runId);
  }, []);

  const handleCloseCancelRunModal = useCallback(() => {
    if (cancelRunSubmitting) return;
    setCancelRunIdToConfirm(null);
    setCancelRunError(null);
  }, [cancelRunSubmitting]);

  const handleConfirmCancelRun = useCallback(async () => {
    if (!cancelRunIdToConfirm || cancelRunSubmitting) return;
    setCancelRunError(null);
    setCancelRunSubmitting(true);
    try {
      await cancelRun(cancelRunIdToConfirm);
      setCancelRunIdToConfirm(null);
      refreshAll();
    } catch (error) {
      console.error(error);
      setCancelRunError(runActionErrorMessage('cancel', cancelRunIdToConfirm, error));
    } finally {
      setCancelRunSubmitting(false);
    }
  }, [cancelRunIdToConfirm, cancelRunSubmitting, refreshAll]);

  const handlePauseRun = useCallback(async (runId: string) => {
    try {
      await pauseRun(runId);
      refreshAll();
    } catch (error) {
      console.error(error);
      setRunActionError(runActionErrorMessage('pause', runId, error));
    }
  }, [refreshAll]);

  const handleResumeRun = useCallback(async (runId: string) => {
    try {
      await resumeRun(runId);
      refreshAll();
    } catch (error) {
      console.error(error);
      setRunActionError(runActionErrorMessage('resume', runId, error));
    }
  }, [refreshAll]);

  return {
    cancelRunIdToConfirm,
    cancelRunSubmitting,
    cancelRunError,
    runActionError,
    setRunActionError,
    handleCancelRun,
    handleCloseCancelRunModal,
    handleConfirmCancelRun,
    handlePauseRun,
    handleResumeRun,
  };
}

// Helpers

function runActionErrorMessage(action: string, runId: string, error: unknown): string {
  return error instanceof Error
    ? `Failed to ${action} run ${runId}: ${error.message}`
    : `Failed to ${action} run ${runId}.`;
}
