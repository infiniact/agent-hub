import { MainShell } from "@/components/layout/MainShell";
import { Header } from "@/components/layout/Header";
import { WorkspaceSidebar } from "@/components/layout/WorkspaceSidebar";
import { PermissionDialogWrapper } from "@/components/chat/PermissionDialogWrapper";
import { ToastContainer } from "@/components/ui/Toast";

export default function Home() {
  return (
    <>
      <Header />
      <div className="flex-1 flex overflow-hidden">
        <WorkspaceSidebar />
        <MainShell />
      </div>
      <PermissionDialogWrapper />
      <ToastContainer />
    </>
  );
}
