package com.jaxrtech.bolt.client;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.googlecode.lanterna.gui2.*;
import com.googlecode.lanterna.gui2.table.Table;
import com.googlecode.lanterna.screen.Screen;
import com.googlecode.lanterna.screen.TerminalScreen;
import com.googlecode.lanterna.terminal.DefaultTerminalFactory;
import com.googlecode.lanterna.terminal.Terminal;
import com.jaxrtech.bolt.BufferContext;
import com.jaxrtech.bolt.Message;
import com.jaxrtech.bolt.MessageContext;
import com.jaxrtech.bolt.MessageReader;
import com.jaxrtech.bolt.MessageRegistration;
import com.jaxrtech.bolt.MessageWriter;
import com.jaxrtech.bolt.messages.*;
import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.IOException;
import java.io.RandomAccessFile;
import java.net.InetSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.SocketChannel;
import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Executors;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.function.Consumer;

class Client {

    private final ObjectMapper objectMapper;
    private final MessageReader messageReader;
    private final BufferContext bufferContext;
    private final MessageWriter writer;
    private final MessageRegistration registration;
    private Selector selector;
    private SocketChannel socket;

    public Client() {
        objectMapper = new ObjectMapper(new MessagePackFactory());
        registration = MessageRegistration.defaultSet();
        messageReader = new MessageReader(registration, objectMapper);
        bufferContext = new BufferContext();
        writer = new MessageWriter(objectMapper);
    }

    public void connect(InetSocketAddress address) throws IOException {
        selector = Selector.open();
        socket = SocketChannel.open(address);
        socket.configureBlocking(true);
        socket.finishConnect();
    }

    public void send(Message message) throws IOException {
        ByteBuffer buffer = bufferContext.getReadBuffer();
        buffer.clear();
        writer.writeTo(message, buffer);
        buffer.flip();

        do {
            socket.write(buffer);
        } while (buffer.remaining() > 0);
    }

    public void configureNonBlocking() throws IOException {
        socket.configureBlocking(false);
        socket.register(selector, SelectionKey.OP_READ);
    }

    public Optional<MessageContext> read() throws IOException {
        selector.select();
        Set<SelectionKey> selectedKeys = selector.selectedKeys();
        Iterator<SelectionKey> iter = selectedKeys.iterator();
        while (iter.hasNext()) {
            SelectionKey key = iter.next();
            iter.remove();

            if (key.isReadable()) {
                return messageReader.read(socket, bufferContext);
            }
        }

        return Optional.empty();
    }

    public MessageContext readUntilNext() throws IOException {
        Optional<MessageContext> result;
        do {
            result = read();
        } while (result.isEmpty());

        return result.get();
    }

    public BufferContext getBufferContext() {
        return bufferContext;
    }
}

class MyWindow extends BasicWindow {

    private final Panel panel;
    private final Label status;
    private final Table<String> fileTable;
    private final List<FileInfo> files = new ArrayList<>();

    public MyWindow() {
        super("BOLT");
        setHints(Arrays.asList(Hint.FULL_SCREEN, Hint.FIT_TERMINAL_WINDOW));

        panel = new Panel();
        panel.setLayoutManager(new LinearLayout(Direction.VERTICAL));

        status = new Label("");
        panel.addComponent(status);

        fileTable = new Table<>("File name", "Size (bytes)");
        fileTable.setVisibleRows(10);
        panel.addComponent(fileTable);

        setComponent(panel);
    }

    public void setStatusText(String text) {
        status.setText(text);
    }

    public void setFiles(List<FileInfo> files) {
        this.files.clear();
        this.files.addAll(files);

        fileTable.getTableModel().clear();
        files.forEach(f -> {
            fileTable.getTableModel().addRow(f.getPath(), Long.toString(f.getSize()));
        });
    }

    public void setFileSelectionListener(Consumer<FileInfo> handler) {
        fileTable.setSelectAction(() -> {
            int index = fileTable.getSelectedRow();
            var file = files.get(index);
            handler.accept(file);
        });
    }
}

interface Action { }

class DownloadFileAction implements Action {
    private final FileInfo file;

    public DownloadFileAction(FileInfo file) {
        this.file = file;
    }

    public FileInfo getFile() {
        return file;
    }
}

class Main {

    private final static BlockingQueue<Message> guiQueue = new LinkedBlockingQueue<>();
    private final static BlockingQueue<Action> actionQueue = new LinkedBlockingQueue<>();
    private static Client client = new Client();

    private final static Map<String, BlockingQueue<FileChunkResponse>> downloadQueues = new HashMap<>();

    public static void main(String[] args) throws Exception {

        ThreadPoolExecutor downloadExecutor = (ThreadPoolExecutor) Executors.newCachedThreadPool();

        Thread tuiThread = new Thread(() -> {
            try {
                runGui();
            } catch (Exception e) {
                e.printStackTrace();
            }
        });

        Thread writeThread = new Thread(() -> {
            try {
                runActions();
            } catch (Exception e) {
                e.printStackTrace();
            }
        });

        tuiThread.start();
        writeThread.start();

        client.connect(new InetSocketAddress("localhost", 9000));
        client.send(new FileListingRequest());
        client.configureNonBlocking();

        while (true) {
            MessageContext envelope = client.readUntilNext();
            Message message = envelope.getMessage();
            if (message instanceof FileListingResponse) {
                guiQueue.offer(message);
            }
            else if (message instanceof FileChunkResponse) {
                FileChunkResponse actualMessage = (FileChunkResponse) message;
                String key = actualMessage.getPath();

                boolean isFresh = downloadQueues.putIfAbsent(key, new LinkedBlockingQueue<>()) == null;
                var queue = downloadQueues.get(key);
                queue.offer(actualMessage);
                if (isFresh) {
                    startDownload(downloadExecutor, queue);
                }
            }
        }
    }

    private static void startDownload(ThreadPoolExecutor downloadExecutor, BlockingQueue<FileChunkResponse> queue) {
        downloadExecutor.submit(() -> {
            try {
                handleDownload(queue);
            } catch (Exception e) {
                e.printStackTrace();
            }
        });
    }

    private static void handleDownload(BlockingQueue<FileChunkResponse> queue) throws InterruptedException, IOException {
        FileChunkResponse chunk = queue.take();
        Path relativePath = Path.of(chunk.getPath());
        Path destinationPath = Path.of(".", "downloads", relativePath.toString());
        var ignored = destinationPath.getParent().toFile().mkdirs();

        System.err.printf("info: [START] downloading '%s' (%s bytes)...%n", chunk.getPath(), chunk.getSize());
        try (var downloadFile = new RandomAccessFile(destinationPath.toFile(), "r")) {
            try (var outputChannel = downloadFile.getChannel()) {
                boolean isDone;
                do {
                    System.err.printf("info: '%s': got %s bytes%n", chunk.getPath(), chunk.getBuffer().limit());
                    outputChannel.write(chunk.getBuffer());
                    isDone = chunk.getOffset() + chunk.getBuffer().limit() < chunk.getSize();
                    if (!isDone) {
                        chunk = queue.take();
                    }
                } while (!isDone);
            }
        }

        System.err.printf("info: [DONE] downloading '%s' (%s bytes)...%n", chunk.getPath(), chunk.getSize());
    }

    private static void runActions() throws InterruptedException, IOException {
        while (true) {
            Action action = actionQueue.take();
            if (action instanceof DownloadFileAction) {
                DownloadFileAction actualAction = (DownloadFileAction) action;
                client.send(new FileFetchRequest(actualAction.getFile().getPath()));
            }
        }
    }

    private static void runGui() throws IOException, InterruptedException {
        Terminal term = new DefaultTerminalFactory().createTerminal();
        Screen screen = new TerminalScreen(term);
        var gui = new MultiWindowTextGUI(screen);
        var window = new MyWindow();
        gui.addWindow(window);
        screen.startScreen();

        screen.doResizeIfNecessary();

        window.setStatusText("Connecting...");
        String spinny = "◢◣◤◥";
        int i = 0;

        window.setFileSelectionListener(f -> {
            actionQueue.offer(new DownloadFileAction(f));
        });

        while (true) {
            gui.getGUIThread().invokeAndWait(() -> {
                try {
                    gui.getGUIThread().processEventsAndUpdate();
                } catch (IOException e) {
                }
            });

            screen.doResizeIfNecessary();

            Message message = guiQueue.poll();
            if (message != null) {
                if (message instanceof FileListingResponse) {
                    var actualMessage = (FileListingResponse) message;
                    window.setFiles(actualMessage.getFiles());
                }
            }

            window.setStatusText(" " + spinny.charAt(i) + " Ready");
            i = (i+1) % spinny.length();

            Thread.sleep(30L);
        }
    }
}