pub mod collections;
pub mod handoff;
pub mod query;
pub mod util;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvError, SyncSender, TrySendError};

use sealed::sealed;
use tuple_list::tuple_list as tl;

use handoff::Handoff;
use handoff::HandoffMeta;
use handoff::NullHandoff;
use handoff::VecHandoff;
use handoff::{CanReceive, TryCanReceive};

use self::handoff::tee::TeeingHandoff;

pub type OpId = usize;
pub type HandoffId = (OpId, OpId);

/**
 * Context provided to a compiled component for writing to an [OutputPort].
 */
pub struct SendCtx<H: Handoff> {
    handoff: Rc<RefCell<H>>,
}
impl<H: Handoff> SendCtx<H> {
    // // TODO: represent backpressure in this return value.
    // #[allow(clippy::result_unit_err)]
    // pub fn give(self, item: H::Item) -> Result<(), ()> {
    //     (*self.once.get()).borrow_mut().try_give(item)
    // }
    pub fn give<T>(&mut self, item: T) -> T
    where
        H: CanReceive<T>,
    {
        let mut borrow = (*self.handoff).borrow_mut();
        <H as CanReceive<T>>::give(&mut *borrow, item)
    }

    pub fn try_give<T>(&mut self, item: T) -> Result<T, T>
    where
        H: TryCanReceive<T>,
    {
        let mut borrow = (*self.handoff).borrow_mut();
        <H as TryCanReceive<T>>::try_give(&mut *borrow, item)
    }
}

/**
 * Handle corresponding to a [SendCtx]. Consumed by [Hydroflow::add_edge] to construct the Hydroflow graph.
 */
#[must_use]
pub struct OutputPort<H: Handoff> {
    op_id: OpId,
    handoff: Rc<RefCell<H>>,
}
impl<H: Handoff> OutputPort<H> {
    pub fn op_id(&self) -> OpId {
        self.op_id
    }
}
impl<T: Clone> Clone for OutputPort<TeeingHandoff<T>> {
    fn clone(&self) -> Self {
        Self {
            op_id: self.op_id,
            handoff: Rc::new(RefCell::new(self.handoff.borrow().tee())),
        }
    }
}

/**
 * Context provided to a compiled component for reading from an [InputPort].
 */
pub struct RecvCtx<H: Handoff> {
    once: Rc<RefCell<Option<Rc<RefCell<H>>>>>,
}
impl<H: Handoff> RecvCtx<H> {
    pub fn take_inner(&mut self) -> H::Inner {
        (*self.once.borrow_mut().as_ref().unwrap().borrow_mut()).take_inner()
    }
}

// impl<T> Iterator for &RecvCtx<VecHandoff<T>> {
//     type Item = T;

//     fn next(&mut self) -> Option<Self::Item> {
//         // TODOTOTODOTOTODTOO !!!!!!!! TODO
//         self.handoff.borrow_mut().0.pop_front()
//     }
// }

/**
 * Handle corresponding to a [RecvCtx]. Consumed by [Hydroflow::add_edge] to construct the Hydroflow graph.
 */
// TODO: figure out how to explain succinctly why this and output port both use Writable
#[must_use]
pub struct InputPort<H: Handoff> {
    op_id: OpId,
    once: Rc<RefCell<Option<Rc<RefCell<H>>>>>,
}
impl<H: Handoff> InputPort<H> {
    pub fn op_id(&self) -> OpId {
        self.op_id
    }
}

/**
 * A variadic list of Handoff types, represented using a lisp-style tuple structure.
 *
 * This trait is sealed and not meant to be implemented or used directly. Instead tuple lists (which already implement this trait) should be used, for example:
 * ```ignore
 * type MyHandoffList = (VecHandoff<usize>, (VecHandoff<String>, (NullHandoff, ())));
 * ```
 * The [`tl!`] (tuple list) macro simplifies usage of this kind:
 * ```ignore
 * type MyHandoffList = tl!(VecHandoff<usize>, VecHandoff<String>, NullHandoff);
 * ```
 */
#[sealed]
pub trait HandoffList {
    type RecvCtx;
    type InputPort;
    type Meta;
    fn make_input(op_id: OpId) -> (Self::RecvCtx, Self::InputPort);

    type SendCtx;
    type OutputPort;
    fn make_output(op_id: OpId) -> (Self::SendCtx, Self::OutputPort);
}
#[sealed]
impl<H, L> HandoffList for (H, L)
where
    H: 'static + Handoff,
    L: HandoffList,
{
    type RecvCtx = (RecvCtx<H>, L::RecvCtx);
    type InputPort = (InputPort<H>, L::InputPort);
    type Meta = (Rc<RefCell<H>>, L::Meta);
    fn make_input(op_id: OpId) -> (Self::RecvCtx, Self::InputPort) {
        let once = Rc::new(RefCell::new(None));

        let recv = RecvCtx { once: once.clone() };
        let input = InputPort { op_id, once };

        let (recv_rest, input_rest) = L::make_input(op_id);

        ((recv, recv_rest), (input, input_rest))
    }

    type SendCtx = (SendCtx<H>, L::SendCtx);
    type OutputPort = (OutputPort<H>, L::OutputPort);
    fn make_output(op_id: OpId) -> (Self::SendCtx, Self::OutputPort) {
        let handoff = Rc::new(RefCell::new(H::default()));

        let send = SendCtx {
            handoff: handoff.clone(),
        };
        let output = OutputPort { op_id, handoff };

        let (send_rest, output_rest) = L::make_output(op_id);

        ((send, send_rest), (output, output_rest))
    }
}
#[sealed]
impl HandoffList for () {
    type RecvCtx = ();
    type InputPort = ();
    type Meta = ();
    fn make_input(_: OpId) -> (Self::RecvCtx, Self::InputPort) {
        ((), ())
    }

    type SendCtx = ();
    type OutputPort = ();
    fn make_output(_: OpId) -> (Self::SendCtx, Self::OutputPort) {
        ((), ())
    }
}

/**
 * Represents a compiled subgraph. Used internally by [Dataflow] to erase the input/output [Handoff] types.
 */
trait Subgraph {
    // TODO: pass in some scheduling info?
    fn run(&mut self);
}
/**
 * Closure-based [OpSubtree] implementation.
 */
struct VariadicClosureSubgraph<F, R, W>
where
    F: FnMut(&mut R::RecvCtx, &mut W::SendCtx),
    R: HandoffList,
    W: HandoffList,
{
    f: F,
    recv: R::RecvCtx,
    send: W::SendCtx,
}
impl<F, R, W> Subgraph for VariadicClosureSubgraph<F, R, W>
where
    F: FnMut(&mut R::RecvCtx, &mut W::SendCtx),
    R: HandoffList,
    W: HandoffList,
{
    fn run(&mut self) {
        (self.f)(&mut self.recv, &mut self.send)
    }
}

struct NtoMClosureSubgraph<F, R, W>
where
    F: FnMut(&mut [RecvCtx<R>], &mut [SendCtx<W>]),
    R: Handoff,
    W: Handoff,
{
    f: F,
    recvs: Vec<RecvCtx<R>>,
    sends: Vec<SendCtx<W>>,
}
impl<F, R, W> Subgraph for NtoMClosureSubgraph<F, R, W>
where
    F: FnMut(&mut [RecvCtx<R>], &mut [SendCtx<W>]),
    R: Handoff,
    W: Handoff,
{
    fn run(&mut self) {
        (self.f)(&mut self.recvs, &mut self.sends)
    }
}

/**
 * A subgraph along with its predacessor and successor [OpId]s.
 * Used internally by the [Hydroflow] struct to represent the dataflow graph structure.
 */
struct SubgraphData {
    subgraph: Box<dyn Subgraph>,
    preds: Vec<OpId>,
    succs: Vec<OpId>,
}
impl SubgraphData {
    pub fn new(subgraph: impl 'static + Subgraph) -> Self {
        Self {
            subgraph: Box::new(subgraph),
            preds: Default::default(),
            succs: Default::default(),
        }
    }
}

/**
 * A handle into a specific [Hydroflow] instance for triggering operators to run, possibly from another thread.Default
 */
#[derive(Clone)]
pub struct Reactor {
    event_queue_send: SyncSender<OpId>,
}
impl Reactor {
    pub fn trigger(&self, op_id: OpId) -> Result<(), TrySendError<usize>> {
        self.event_queue_send.try_send(op_id)
    }

    #[cfg(feature = "async")]
    pub fn into_waker(self, op_id: OpId) -> std::task::Waker {
        use futures::task::ArcWake;
        use std::sync::Arc;

        struct ReactorWaker {
            reactor: Reactor,
            op_id: OpId,
        }
        impl ArcWake for ReactorWaker {
            fn wake_by_ref(arc_self: &Arc<Self>) {
                arc_self.reactor.trigger(arc_self.op_id).unwrap(/* TODO(mingwei) */);
            }
        }

        let reactor_waker = ReactorWaker {
            reactor: self,
            op_id,
        };
        futures::task::waker(Arc::new(reactor_waker))
    }
}

/**
 * A Hydroflow graph. Owns, schedules, and runs the compiled subgraphs.
 */
pub struct Hydroflow {
    subgraphs: Vec<SubgraphData>,
    handoffs: HashMap<HandoffId, Box<dyn HandoffMeta>>,

    // TODO(mingwei): separate scheduler into its own struct/trait?
    ready_queue: VecDeque<OpId>,
    event_queue_send: SyncSender<OpId>, // TODO(mingwei) remove this, to prevent hanging.
    event_queue_recv: Receiver<OpId>,
}
impl Default for Hydroflow {
    fn default() -> Self {
        let (subgraphs, handoffs, ready_queue) = Default::default();
        let (event_queue_send, event_queue_recv) = mpsc::sync_channel(8_000);
        Self {
            subgraphs,
            handoffs,
            ready_queue,
            event_queue_send,
            event_queue_recv,
        }
    }
}
impl Hydroflow {
    /**
     * Create an new empty Dataflow graph.
     */
    pub fn new() -> Self {
        Default::default()
    }

    /**
     * Returns a reactor for externally scheduling operators, possibly from another thread.
     */
    pub fn reactor(&self) -> Reactor {
        Reactor {
            event_queue_send: self.event_queue_send.clone(),
        }
    }

    /**
     * Runs the dataflow until no more work is currently available.
     */
    pub fn tick(&mut self) {
        loop {
            // Add any external jobs to ready queue.
            self.ready_queue.extend(self.event_queue_recv.try_iter());

            if let Some(op_id) = self.ready_queue.pop_front() {
                let sg_data = self.subgraphs.get_mut(op_id).unwrap(/* TODO(mingwei) */);
                sg_data.subgraph.run();
                for succ_id in sg_data.succs.iter().copied() {
                    if self.ready_queue.contains(&succ_id) {
                        // TODO(mingwei): Slow? O(N)
                        continue;
                    }
                    let handoff_id: HandoffId = (op_id, succ_id);
                    let handoff = self.handoffs.get(&handoff_id).unwrap(/* TODO(mingwei) */);
                    if !handoff.is_bottom() {
                        self.ready_queue.push_back(succ_id);
                    }
                }
            } else {
                break;
            }
        }
    }

    /**
     * Run the dataflow graph, blocking until completion.
     */
    pub fn run(&mut self) -> Result<!, RecvError> {
        loop {
            // Do any current work.
            self.tick();

            // Block and wait for an external event.
            self.ready_queue.push_back(self.event_queue_recv.recv()?);
        }
    }

    /**
     * TODO(mingwei): Hack to re-enqueue all subgraphs.
     */
    pub fn wake_all(&mut self) {
        self.ready_queue.clear();
        self.ready_queue.extend(0..self.subgraphs.len());
    }

    /**
     * Adds a new compiled subgraph with the specified inputs and outputs.
     *
     * See [HandoffList] for how to specify inputs and outputs.
     */
    #[must_use]
    pub fn add_subgraph<F, R, W>(&mut self, subgraph: F) -> (R::InputPort, W::OutputPort)
    where
        F: 'static + FnMut(&mut R::RecvCtx, &mut W::SendCtx),
        R: 'static + HandoffList,
        W: 'static + HandoffList,
    {
        // TODO(justin): make this less sketchy, we just know we're the only person who will append here.
        let op_id = self.subgraphs.len();

        let (recv, input_port) = R::make_input(op_id);
        let (send, output_port) = W::make_output(op_id);

        let sg: VariadicClosureSubgraph<F, R, W> = VariadicClosureSubgraph {
            f: subgraph,
            recv,
            send,
        };

        self.subgraphs.push(SubgraphData::new(sg));
        self.ready_queue.push_back(op_id);

        (input_port, output_port)
    }

    /**
     * Adds a new compiled subraph with a single input and output, and returns the input/output handles.
     */
    pub fn add_inout<F, R, W>(&mut self, mut subgraph: F) -> (InputPort<R>, OutputPort<W>)
    where
        F: 'static + FnMut(&mut RecvCtx<R>, &mut SendCtx<W>),
        R: 'static + Handoff,
        W: 'static + Handoff,
    {
        let (tl!(input_port), tl!(output_port)) = self
            .add_subgraph::<_, tl!(R), tl!(W)>(move |tl!(recv), tl!(send)| (subgraph)(recv, send));
        (input_port, output_port)
    }

    /**
     * Adds a new compiled subraph with one input and two outputs, and returns the input/output handles.
     */
    pub fn add_binary_out<F, R, W1, W2>(
        &mut self,
        mut subgraph: F,
    ) -> (InputPort<R>, OutputPort<W1>, OutputPort<W2>)
    where
        F: 'static + FnMut(&mut RecvCtx<R>, &mut SendCtx<W1>, &mut SendCtx<W2>),
        R: 'static + Handoff,
        W1: 'static + Handoff,
        W2: 'static + Handoff,
    {
        let (tl!(input_port), tl!(output_port1, output_port2)) = self
            .add_subgraph::<_, tl!(R), tl!(W1, W2)>(move |tl!(recv), tl!(send1, send2)| {
                (subgraph)(recv, send1, send2)
            });
        (input_port, output_port1, output_port2)
    }

    /**
     * Adds a new compiled subraph with a variable number of inputs and outputs.
     */
    pub fn add_n_in_m_out<F, R, W>(
        &mut self,
        n: usize,
        m: usize,
        subgraph: F,
    ) -> (Vec<InputPort<R>>, Vec<OutputPort<W>>)
    where
        F: 'static + FnMut(&mut [RecvCtx<R>], &mut [SendCtx<W>]),
        R: 'static + Handoff,
        W: 'static + Handoff,
    {
        let mut recvs = Vec::new();
        let mut input_ports = Vec::new();
        let mut input_metas = Vec::new();
        let op_id = self.subgraphs.len();

        for _ in 0..n {
            let handoff = Rc::new(RefCell::new(R::default()));
            let once = Rc::new(RefCell::new(None));
            recvs.push(RecvCtx { once: once.clone() });
            input_ports.push(InputPort { once, op_id });
            input_metas.push(Box::new(handoff) as Box<dyn HandoffMeta>);
        }

        let mut sends = Vec::new();
        let mut output_ports = Vec::new();

        for _ in 0..m {
            let handoff = Rc::new(RefCell::new(W::default()));

            sends.push(SendCtx {
                handoff: handoff.clone(),
            });
            output_ports.push(OutputPort { op_id, handoff });
        }

        self.subgraphs.push(SubgraphData::new(NtoMClosureSubgraph {
            f: subgraph,
            recvs,
            sends,
        }));
        self.ready_queue.push_back(op_id);

        (input_ports, output_ports)
    }

    /**
     * Adds a new compiled subraph with one input and two outputs, and returns the input/output handles.
     */
    pub fn add_binary_in_binary_out<F, R1, R2, W1, W2>(
        &mut self,
        mut subgraph: F,
    ) -> (InputPort<R1>, InputPort<R2>, OutputPort<W1>, OutputPort<W2>)
    where
        F: 'static + FnMut(&mut RecvCtx<R1>, &mut RecvCtx<R2>, &mut SendCtx<W1>, &mut SendCtx<W2>),
        R1: 'static + Handoff,
        R2: 'static + Handoff,
        W1: 'static + Handoff,
        W2: 'static + Handoff,
    {
        let (tl!(input_port1, input_port2), tl!(output_port1, output_port2)) = self
            .add_subgraph::<_, tl!(R1, R2), tl!(W1, W2)>(
                move |tl!(recv1, recv2), tl!(send1, send2)| (subgraph)(recv1, recv2, send1, send2),
            );
        (input_port1, input_port2, output_port1, output_port2)
    }

    /**
     * Adds a new compiled subraph with two inputs and a single output, and returns the input/output handles.
     */
    pub fn add_binary<F, R1, R2, W>(
        &mut self,
        mut subgraph: F,
    ) -> (InputPort<R1>, InputPort<R2>, OutputPort<W>)
    where
        F: 'static + FnMut(&mut RecvCtx<R1>, &mut RecvCtx<R2>, &mut SendCtx<W>),
        R1: 'static + Handoff,
        R2: 'static + Handoff,
        W: 'static + Handoff,
    {
        let (tl!(input_port1, input_port2), tl!(output_port)) = self
            .add_subgraph::<_, tl!(R1, R2), tl!(W)>(move |tl!(recv1, recv2), tl!(send)| {
                (subgraph)(recv1, recv2, send)
            });
        (input_port1, input_port2, output_port)
    }

    /**
     * Adds a new compiled subraph with two inputs and no outputs, and returns the input handles.
     */
    pub fn add_binary_sink<F, R1, R2>(&mut self, mut subgraph: F) -> (InputPort<R1>, InputPort<R2>)
    where
        F: 'static + FnMut(&mut RecvCtx<R1>, &mut RecvCtx<R2>),
        R1: 'static + Handoff,
        R2: 'static + Handoff,
    {
        let (tl!(input_port1, input_port2), tl!()) =
            self.add_subgraph::<_, tl!(R1, R2), tl!()>(move |tl!(recv1, recv2), tl!()| {
                (subgraph)(recv1, recv2)
            });
        (input_port1, input_port2)
    }

    /**
     * Adds a new compiled subgraph with no inputs and one output.
     */
    pub fn add_source<F, W>(&mut self, mut subgraph: F) -> OutputPort<W>
    where
        F: 'static + FnMut(&mut SendCtx<W>),
        W: 'static + Handoff,
    {
        self.add_inout::<_, NullHandoff, W>(move |_, send| subgraph(send))
            .1
    }

    /**
     * Adds a new compiled subgraph with one inputs and no outputs.
     */
    pub fn add_sink<F, R>(&mut self, mut subgraph: F) -> InputPort<R>
    where
        F: 'static + FnMut(&mut RecvCtx<R>),
        R: 'static + Handoff,
    {
        self.add_inout::<_, R, NullHandoff>(move |recv, _| subgraph(recv))
            .0
    }

    pub fn add_edge<H>(&mut self, output_port: OutputPort<H>, input_port: InputPort<H>)
    where
        H: 'static + Handoff,
    {
        // Add successor.
        self.subgraphs[output_port.op_id]
            .succs
            .push(input_port.op_id);
        // Add predacessor.
        self.subgraphs[input_port.op_id]
            .preds
            .push(output_port.op_id);
        // Insert handoff.
        let handoff_id: HandoffId = (output_port.op_id, input_port.op_id);
        self.handoffs
            .insert(handoff_id, Box::new(output_port.handoff.clone()));

        *input_port.once.borrow_mut() = Some(output_port.handoff);
    }
}

#[test]
fn map_filter() {
    // A simple dataflow with one source feeding into one sink with some processing in the middle.
    let mut df = Hydroflow::new();

    let data = [1, 2, 3, 4];
    let source = df.add_source(move |send| {
        for x in data.into_iter() {
            send.give(Some(x));
        }
    });

    let (map_in, map_out) = df.add_inout(
        |recv: &mut RecvCtx<VecHandoff<i32>>, send: &mut SendCtx<VecHandoff<_>>| {
            for x in recv.take_inner().into_iter() {
                send.give(Some(3 * x + 1));
            }
        },
    );

    let (filter_in, filter_out) = df.add_inout(
        |recv: &mut RecvCtx<VecHandoff<i32>>, send: &mut SendCtx<VecHandoff<_>>| {
            for x in recv.take_inner().into_iter() {
                if x % 2 == 0 {
                    send.give(Some(x));
                }
            }
        },
    );

    let outputs = Rc::new(RefCell::new(Vec::new()));
    let inner_outputs = outputs.clone();
    let sink = df.add_sink(move |recv: &mut RecvCtx<VecHandoff<i32>>| {
        for x in recv.take_inner().into_iter() {
            (*inner_outputs).borrow_mut().push(x);
        }
    });

    df.add_edge(source, map_in);
    df.add_edge(map_out, filter_in);
    df.add_edge(filter_out, sink);

    df.tick();

    assert_eq!((*outputs).borrow().clone(), vec![4, 10]);
}

mod tests {
    #![allow(unused_imports)]
    use std::{
        cell::RefCell,
        collections::{HashMap, HashSet},
        rc::Rc,
    };

    use crate::scheduled::{handoff::Handoff, Hydroflow, RecvCtx, SendCtx, VecHandoff};

    #[test]
    fn test_cycle() {
        // A dataflow that represents graph reachability.

        let mut edges: HashMap<usize, Vec<usize>> = HashMap::new();
        for (from, to) in &[
            (1_usize, 2_usize),
            (1, 3),
            (1, 4),
            (2, 3),
            (2, 5),
            (5, 1),
            (6, 7),
            (7, 8),
        ] {
            edges.entry(*from).or_insert_with(Vec::new).push(*to);
        }

        let mut df = Hydroflow::new();

        let mut initially_reachable = vec![1];
        let reachable = df.add_source(move |send: &mut SendCtx<VecHandoff<usize>>| {
            for v in initially_reachable.drain(..) {
                send.give(Some(v));
            }
        });

        let mut seen = HashSet::new();
        let (distinct_in, distinct_out) = df.add_inout(
            move |recv: &mut RecvCtx<VecHandoff<usize>>, send: &mut SendCtx<VecHandoff<usize>>| {
                for v in recv.take_inner().into_iter() {
                    if seen.insert(v) {
                        send.give(Some(v));
                    }
                }
            },
        );

        let (merge_lhs, merge_rhs, merge_out) = df.add_binary(
            |recv1: &mut RecvCtx<VecHandoff<usize>>,
             recv2: &mut RecvCtx<VecHandoff<usize>>,
             send: &mut SendCtx<VecHandoff<usize>>| {
                for v in (recv1.take_inner().into_iter()).chain(recv2.take_inner().into_iter()) {
                    send.give(Some(v));
                }
            },
        );

        let (neighbors_in, neighbors_out) =
            df.add_inout(move |recv: &mut RecvCtx<VecHandoff<usize>>, send| {
                for v in recv.take_inner().into_iter() {
                    if let Some(neighbors) = edges.get(&v) {
                        for &n in neighbors {
                            send.give(Some(n));
                        }
                    }
                }
            });

        let (tee_in, tee_out1, tee_out2) = df.add_binary_out(
            |recv: &mut RecvCtx<VecHandoff<usize>>,
             send1: &mut SendCtx<VecHandoff<usize>>,
             send2: &mut SendCtx<VecHandoff<usize>>| {
                for v in recv.take_inner().into_iter() {
                    send1.give(Some(v));
                    send2.give(Some(v));
                }
            },
        );

        let reachable_verts = Rc::new(RefCell::new(Vec::new()));
        let reachable_inner = reachable_verts.clone();
        let sink_in = df.add_sink(move |recv: &mut RecvCtx<VecHandoff<usize>>| {
            for v in recv.take_inner().into_iter() {
                (*reachable_inner).borrow_mut().push(v);
            }
        });

        df.add_edge(reachable, merge_lhs);
        df.add_edge(neighbors_out, merge_rhs);
        df.add_edge(merge_out, distinct_in);
        df.add_edge(distinct_out, tee_in);
        df.add_edge(tee_out1, neighbors_in);
        df.add_edge(tee_out2, sink_in);

        df.tick();

        assert_eq!((*reachable_verts).borrow().clone(), vec![1, 2, 3, 4, 5]);
    }
}

#[test]
fn test_auto_tee() {
    let mut df = Hydroflow::new();

    let mut data = vec![1, 2, 3, 4];
    let source = df.add_source(move |send: &mut SendCtx<TeeingHandoff<_>>| {
        send.give(std::mem::take(&mut data));
    });

    let out1 = Rc::new(RefCell::new(Vec::new()));
    let out1_inner = out1.clone();

    let sink1 = df.add_sink(move |recv: &mut RecvCtx<_>| {
        for v in recv.take_inner() {
            out1_inner.borrow_mut().extend(v);
        }
    });

    let out2 = Rc::new(RefCell::new(Vec::new()));
    let out2_inner = out2.clone();
    let sink2 = df.add_sink(move |recv: &mut RecvCtx<_>| {
        for v in recv.take_inner() {
            out2_inner.borrow_mut().extend(v);
        }
    });

    df.add_edge(source.clone(), sink1);
    df.add_edge(source, sink2);

    df.tick();

    assert_eq!((*out1).borrow().clone(), vec![1, 2, 3, 4]);
    assert_eq!((*out2).borrow().clone(), vec![1, 2, 3, 4]);
}
