import { MainShell } from "@/components/layout/MainShell";
import { Header } from "@/components/layout/Header";
import { PermissionDialogWrapper } from "@/components/chat/PermissionDialogWrapper";

export default function Home() {
  return (
    <>
      <Header />
      <MainShell />
      <PermissionDialogWrapper />
    </>
  );
}
