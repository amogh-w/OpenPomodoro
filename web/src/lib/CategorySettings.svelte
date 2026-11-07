<script lang="ts">
  import CategoryList from "./CategoryList.svelte";
  import { t } from "./i18n";
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 p-4" aria-labelledby="categories-title">
  <h2 id="categories-title" class="t-title3">{t("categories.title")}</h2>
  <CategoryList />
</section>
