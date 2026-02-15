export interface Workspace {
  id: string;
  name: string;
  icon: string;
  working_directory: string;
  created_at: string;
  updated_at: string;
}

export interface CreateWorkspaceRequest {
  name: string;
  icon?: string;
  working_directory?: string;
  agent_ids?: string[];
}

export interface UpdateWorkspaceRequest {
  name?: string;
  icon?: string;
  working_directory?: string;
}
