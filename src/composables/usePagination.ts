import { computed, ref } from "vue";

export function usePagination<T>(source: () => T[]) {
  const page = ref(1);
  const pageSize = ref(20);

  const paged = computed(() => {
    const start = (page.value - 1) * pageSize.value;
    return source().slice(start, start + pageSize.value);
  });

  return { page, pageSize, paged };
}