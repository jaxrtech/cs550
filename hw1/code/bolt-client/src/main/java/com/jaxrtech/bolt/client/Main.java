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
import com.jaxrtech.bolt.messages.FileInfo;
import com.jaxrtech.bolt.messages.FileListingRequest;
import com.jaxrtech.bolt.messages.FileListingResponse;
import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.net.InetSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.SocketChannel;
import java.util.*;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;

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

    public void write(Message message) throws IOException {
        ByteBuffer buffer = bufferContext.getReadBuffer();
        writer.writeTo(message, buffer);
        buffer.flip();

        do {
            socket.write(buffer);
        } while (buffer.remaining() > 0);

        buffer.clear();
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
        fileTable.getTableModel().clear();
        files.forEach(f -> {
            fileTable.getTableModel().addRow(f.getName(), Long.toString(f.getSize()));
        });
    }
}

class Main {

    private final static BlockingQueue<Message> guiQueue = new LinkedBlockingQueue<>();

    public static void main(String[] args) throws Exception {

        Thread tuiThread = new Thread(() -> {
            try {
                run();
            } catch (Exception e) {
                e.printStackTrace();
            }
        });
        tuiThread.start();

        var client = new Client();
        client.connect(new InetSocketAddress("localhost", 9000));
        client.write(new FileListingRequest());
        client.configureNonBlocking();

        while (true) {
            MessageContext envelope = client.readUntilNext();

            Message message = envelope.getMessage();
            if (message.getKind().equals("FILES")) {
                guiQueue.offer(message);
            }

            // TODO: Ask user to pick files to download
            break;
        }

        tuiThread.join();
    }

    private static void run() throws IOException, InterruptedException {
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