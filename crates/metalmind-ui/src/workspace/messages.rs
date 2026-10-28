//! Los avisos de la ventana: los que se ven como toasts y el historial que
//! lista `:messages`, como el de vim.

use std::collections::VecDeque;

use super::Message;

/// Cuántos avisos guarda el historial, como `messagesopt=history:500` de nvim;
/// el más viejo se va, salvo si se ve.
const HISTORY: usize = 500;
/// Cuántos toasts se ven a la vez; uno nuevo cierra el más viejo.
const TOASTS: usize = 4;

/// Un aviso del historial. Crecen con cada aviso: el de un aviso que se
/// vuelve a decir es otro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId(u64);

/// Una vez que se muestra un aviso: volver a mostrarlo da otro, así su
/// temporizador empieza de nuevo y el viejo ya no lo cierra.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToastId(u64);

#[derive(Debug, Default)]
pub struct Messages {
    /// Del más viejo al más nuevo.
    log: VecDeque<Logged>,
    next: u64,
    /// Cuántas veces se abrió un toast: ordena los que se ven.
    shows: u64,
}

#[derive(Debug)]
struct Logged {
    id: MessageId,
    message: Message,
    /// Su toast, si se ve.
    shown: Option<ToastId>,
}

impl Messages {
    /// Lo anota y lo muestra en un toast. Si ya se veía uno igual, lo
    /// reemplaza: repetir `n` no apila avisos.
    pub fn push(&mut self, message: Message) -> MessageId {
        for logged in &mut self.log {
            if logged.shown.is_some() && logged.message == message {
                logged.shown = None;
            }
        }
        let id = self.record(message);
        self.show(id);
        id
    }

    /// Lo anota en el historial sin mostrarlo, como el `"a.md" 3L, 20B` de
    /// cada nota que se abre.
    pub fn record(&mut self, message: Message) -> MessageId {
        let id = MessageId(self.next);
        self.next += 1;
        if self.log.len() == HISTORY
            && let Some(oldest) = self.log.iter().position(|logged| logged.shown.is_none())
        {
            self.log.remove(oldest);
        }
        self.log.push_back(Logged {
            id,
            message,
            shown: None,
        });
        id
    }

    /// Vuelve a mostrar uno del historial, como el último toast.
    pub fn show(&mut self, id: MessageId) {
        self.shows += 1;
        let toast = ToastId(self.shows);
        let Some(logged) = self.log.iter_mut().find(|logged| logged.id == id) else {
            return;
        };
        logged.shown = Some(toast);
        let mut shown: Vec<&mut Logged> = self
            .log
            .iter_mut()
            .filter(|logged| logged.shown.is_some())
            .collect();
        if shown.len() > TOASTS {
            shown.sort_by_key(|logged| logged.shown);
            let extra = shown.len() - TOASTS;
            for logged in shown.into_iter().take(extra) {
                logged.shown = None;
            }
        }
    }

    /// Cierra ese toast, si sigue abierto; el aviso sigue en el historial.
    pub fn dismiss(&mut self, toast: ToastId) {
        if let Some(logged) = self
            .log
            .iter_mut()
            .find(|logged| logged.shown == Some(toast))
        {
            logged.shown = None;
        }
    }

    pub fn dismiss_all(&mut self) {
        for logged in &mut self.log {
            logged.shown = None;
        }
    }

    /// Los que se ven, del que se abrió antes al último.
    #[must_use]
    pub fn toasts(&self) -> Vec<(ToastId, &Message)> {
        let mut shown: Vec<(ToastId, &Message)> = self
            .log
            .iter()
            .filter_map(|logged| Some((logged.shown?, &logged.message)))
            .collect();
        shown.sort_by_key(|&(toast, _)| toast);
        shown
    }

    /// Todo lo anotado, del más nuevo al más viejo.
    pub fn history(&self) -> impl Iterator<Item = (MessageId, &Message)> {
        self.log
            .iter()
            .rev()
            .map(|logged| (logged.id, &logged.message))
    }

    /// El último anotado, se vea o no.
    #[must_use]
    pub fn last(&self) -> Option<&Message> {
        self.log.back().map(|logged| &logged.message)
    }

    /// Para preguntar después con [`Messages::since`] qué se anotó entre
    /// medio.
    #[must_use]
    pub fn mark(&self) -> MessageId {
        MessageId(self.next)
    }

    /// Lo anotado desde `mark`, del más viejo al más nuevo.
    pub fn since(&self, mark: MessageId) -> impl Iterator<Item = &Message> {
        self.log
            .iter()
            .filter(move |logged| logged.id >= mark)
            .map(|logged| &logged.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(text: &str) -> Message {
        Message::Info(text.to_owned())
    }

    fn shown(messages: &Messages) -> Vec<&str> {
        messages
            .toasts()
            .into_iter()
            .map(|(_, message)| message.text())
            .collect()
    }

    #[test]
    fn cerrar_un_toast_lo_deja_en_el_historial() {
        let mut messages = Messages::default();
        messages.push(info("uno"));
        messages.push(info("dos"));
        let (uno, _) = messages.toasts()[0];
        messages.dismiss(uno);
        assert_eq!(shown(&messages), ["dos"]);
        let history: Vec<&str> = messages.history().map(|(_, m)| m.text()).collect();
        assert_eq!(history, ["dos", "uno"]);
        messages.dismiss_all();
        assert_eq!(messages.toasts().len(), 0);
        assert_eq!(messages.history().count(), 2);
    }

    #[test]
    fn uno_nuevo_cierra_el_mas_viejo_pasado_el_maximo() {
        let mut messages = Messages::default();
        for text in ["1", "2", "3", "4", "5"] {
            messages.push(info(text));
        }
        assert_eq!(shown(&messages), ["2", "3", "4", "5"]);
    }

    #[test]
    fn repetir_un_aviso_que_se_ve_no_lo_apila() {
        let mut messages = Messages::default();
        let primero = messages.push(info("dio la vuelta"));
        messages.push(info("otro"));
        let segundo = messages.push(info("dio la vuelta"));
        assert_ne!(primero, segundo);
        assert_eq!(shown(&messages), ["otro", "dio la vuelta"]);
        assert_eq!(messages.history().count(), 3, "el historial guarda los dos");
    }

    #[test]
    fn uno_del_historial_vuelve_como_el_ultimo_toast() {
        let mut messages = Messages::default();
        let viejo = messages.push(info("viejo"));
        messages.push(info("nuevo"));
        messages.dismiss_all();
        messages.show(viejo);
        messages.push(info("otro"));
        assert_eq!(shown(&messages), ["viejo", "otro"]);
    }

    #[test]
    fn volver_a_mostrar_uno_que_se_ve_lo_vuelve_otro_toast() {
        let mut messages = Messages::default();
        let id = messages.push(info("uno"));
        let (antes, _) = messages.toasts()[0];
        messages.show(id);
        let (ahora, _) = messages.toasts()[0];
        assert_ne!(antes, ahora);
        messages.dismiss(antes);
        assert_eq!(shown(&messages), ["uno"], "el toast viejo ya no lo cierra");
        messages.dismiss(ahora);
        assert_eq!(messages.toasts().len(), 0);
    }

    #[test]
    fn lo_anotado_sin_mostrar_solo_va_al_historial() {
        let mut messages = Messages::default();
        messages.record(info("\"a.md\" 3L, 20B"));
        assert_eq!(messages.toasts().len(), 0);
        assert_eq!(messages.last(), Some(&info("\"a.md\" 3L, 20B")));
    }

    #[test]
    fn since_dice_lo_anotado_despues_de_la_marca() {
        let mut messages = Messages::default();
        messages.push(info("antes"));
        let mark = messages.mark();
        assert_eq!(messages.since(mark).count(), 0);
        messages.push(Message::Error("E492".to_owned()));
        let since: Vec<&str> = messages.since(mark).map(Message::text).collect();
        assert_eq!(since, ["E492"]);
    }

    #[test]
    fn el_historial_tiene_tope() {
        let mut messages = Messages::default();
        for n in 0..=HISTORY {
            messages.record(info(&n.to_string()));
        }
        assert_eq!(messages.history().count(), HISTORY);
        assert_eq!(messages.history().last().map(|(_, m)| m.text()), Some("1"));
    }

    #[test]
    fn un_toast_abierto_no_se_va_con_el_historial() {
        let mut messages = Messages::default();
        messages.push(Message::Error("E492".to_owned()));
        for n in 0..HISTORY {
            messages.record(info(&n.to_string()));
        }
        assert_eq!(shown(&messages), ["E492"]);
        assert_eq!(messages.history().count(), HISTORY);
    }

    #[test]
    fn los_errores_no_se_cierran_solos() {
        assert_eq!(Message::Error(String::new()).lifetime(), None);
        assert!(
            Message::Info(String::new()).lifetime() < Message::Warning(String::new()).lifetime()
        );
    }
}
