export function Pane({ name }: { name: string }) {
  const [open, setOpen] = useState(false);
  return <div className="pane" onClick={() => setOpen(!open)}>{name}</div>;
}
