import { useEffect, useRef } from "react";

export function Confirm({ message, action, cancel, confirm }: {
  message: string; action: string; cancel: () => void; confirm: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => { dialog.current?.showModal(); }, []);

  return <dialog ref={dialog} className="confirm-dialog" aria-label={action}
    onCancel={(event) => { event.preventDefault(); cancel(); }}>
    <p>{message}</p>
    <div className="row form-actions">
      <button autoFocus onClick={cancel}>取消</button>
      <button className="primary" onClick={confirm}>{action}</button>
    </div>
  </dialog>;
}
