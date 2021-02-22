<script lang="typescript">
  import { goGov, proposals } from "../src/funding/contract";

  import { SidebarLayout } from "../DesignSystem/Component";

  async function go() {
    await goGov();
  }

  go();
</script>

<style>
  .container {
    max-width: var(--content-max-width);
    margin: 64px auto;
    min-width: var(--content-min-width);
    padding: 0 var(--content-padding);
  }
</style>

<SidebarLayout dataCy="governance-page">
  <div class="container">Hello! This is the governance screen.</div>

  {JSON.stringify($proposals)}
  <ul>
    {#each $proposals as p}
      <li>{JSON.stringify(p)}</li>
    {/each}
  </ul>
</SidebarLayout>
