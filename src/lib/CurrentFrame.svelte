<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';
  import type { Frame, Project } from '../backend';

  let errormessage: string|undefined = undefined;
  let frame: Frame|undefined = undefined;
  let project: Project|undefined = undefined;

  async function current_frame() {
    try {
      frame = await invoke('current_frame');
      console.log(frame);
      if (frame) {
        project = await invoke('lookup_project', {projectId: frame.project});
        console.log(project);
        errormessage = undefined;
      } else {
        project = undefined
      }

    } catch (e: any) {
      errormessage = e.toString();
    }
  }
</script>

<div>
  <button on:click="{current_frame}">Show</button>
  <p>{project?.name ?? 'Not running'}</p>
  <p>{frame?.start ?? 'Not running'}</p>
  <p>{errormessage ?? ''}</p>
</div>