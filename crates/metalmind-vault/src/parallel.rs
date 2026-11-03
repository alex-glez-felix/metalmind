use std::num::NonZeroUsize;
use std::thread;

/// `work` sobre cada elemento, repartido en un hilo por núcleo, con los
/// resultados en el orden de `items`.
pub(crate) fn in_parallel<T: Send, R: Send>(items: Vec<T>, work: impl Fn(T) -> R + Sync) -> Vec<R> {
    let threads = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let chunk = items.len().div_ceil(threads).max(1);
    let mut chunks = Vec::new();
    let mut items = items.into_iter();
    loop {
        let part: Vec<T> = items.by_ref().take(chunk).collect();
        if part.is_empty() {
            break;
        }
        chunks.push(part);
    }
    let work = &work;
    thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|part| scope.spawn(move || part.into_iter().map(work).collect::<Vec<R>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conserva_el_orden() {
        let items: Vec<usize> = (0..500).collect();
        assert_eq!(in_parallel(items.clone(), |item| item), items);
    }
}
