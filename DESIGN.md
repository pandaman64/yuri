# Yuriでどのようなコードが生成されるのか

Yuriの関数は全て中断可能で，スタックレスコルーチンとしてコンパイルされる（なので`async/await`は無い．関数呼び出しが暗黙に中断ポイントと認識される）．
スタックレスコルーチンは関数の実行に必要なステートとステップ関数（普通の意味で）の組として表される．


```rust
fn copy (src: Fd) (dest: Fd) {
    let mut buf = [0u8; 1024];

    loop {
        let n = read(src, &mut buf);
        if n == 0 {
            break;
        }
        write(dest, &buf[0..n]);
    }
}
```

とプログラマが書いたとする．まずは関数呼び出しをステートマシンの駆動に脱糖する．
処理が完了するまで`yield`することでその間他の関数が実行できるようになるよ．非同期だね．

```rust
fn copy (src: Fd) (dest: Fd) {
    let mut buf = [0u8; 1024];

    loop {
        // read()用のステートマシン（ミニ・スタック）を作る
        let mut read__stack = create_read__stack(src, &mut buf);

        // read()の完了を待つ
        let n = loop {
            match read__stack.poll() {
                Poll::Ready(n) => break n,
                Poll::Pending => yield,
            }
        };

        if n == 0 {
            // ステートマシンの破棄（ポイント: dropが非同期的！）
            // ここではread側しか存在しないのでそれだけ
            loop {
                match write__stack.drop() {
                    Poll::Ready(()) => break,
                    Poll::Pending => yield,
                }
            }
            break;
        }

        // write()用のステートマシン
        let mut write__stack = create_write__stack(dest, &mut buf);

        // write()完了待ち
        loop {
            match write__stack.poll() {
                Poll::Ready(()) => break,
                Poll::Pending => yield,
            }
        }

        // ステートマシンを破棄する
        loop {
            match write__stack.drop() {
                Poll::Ready(()) => break,
                Poll::Pending => yield,
            }
        }
        loop {
            match read__stack.drop() {
                Poll::Ready(()) => break,
                Poll::Pending => yield,
            }
        }
    }
}
```

なんと`drop`の呼び出しも非同期化されて完了するまで`yield`するようになってるよ！
だからio_uring (Linux)やIOCP (Windows)のようなキャンセル自体も非同期処理もキレイに扱えるようになるんだ．
ここがRustとは違うところなんだね～～～（Rustは`drop`をブロックせずに待つということができない．だからここが狙い目）

ここから更に`copy`自体もステートマシン化していく．基本的には`yield`ごとに関数を細切れにして`yield`のところで`return Pending`する．
ステートマシンには引数と`yield`をまたぐような変数だけ格納すればよい（スタック全体が要らないのでスタックレス）．

例: `n`は`yield`をまたがないのでステートマシンに含まれない．

本当はdisjointな区間でのみ使われるスロットは再利用できるけどそれは最適化ということで．

```rust
enum copy__state {
    Init, // 実行前
    ReadWait, // readの完了待ち
    WriteWait, // writeの完了待ち
}
struct copy__stack {
    state: copy__state,
    buf: [u8; 1024],
    src: Fd,
    dest: Fd,
    read__stack: read__stack,
    write__stack: write__stack,
}

fn create_copy__stack (src: Fd) (dest: Fd): copy__stack {
    copy__stack {
        state: Init,
        src,
        dest,
        // 後は未初期化
    }
}

fn step_copy (st: &mut copy_stack): Poll<()> {
    // waitせずに操作が完了した場合はstateだけ変更してループ回しなおす
    loop {
        match st.state {
            Init => {
                st.buf = [0u8; 1024];
                st.read__stack = create_read__stack(st.src, &mut st.buf);
                st.state = ReadWait;
            }
            ReadWait => {
                let n = match st.read__stack.poll() {
                    Poll::Ready(n) => n,
                    Poll::Pending => return Poll::Pending,
                };

                if n == 0 {
                    return Poll::Ready(());
                }

                st.write__stack = create_write__stack(st.dest, &st.buf[0..n]);
                st.state = WriteWait;
            }
            WriteWait => {
                match st.write__stack.poll() {
                    Poll::Ready(()) => {},
                    Poll::Pending => return Poll::Pending,
                }

                st.read__stack = create_read__stack(st.src, &mut st.buf);
                st.state = ReadWait;
            }
        }
    }
}
```

手でやってみて思ったけどステートマシン変換かなり非自明だな．


最初の`yield`を使った脱糖のところでは`drop`の呼び出しも埋め込んだけど，本当は`drop`は専用の関数に移される．
内部のフィールドに対して再帰的に`drop`を呼び出して全て完了するまで待機する．

```rust
fn drop_copy__stack (st: &mut copy__stack): Poll<()> {
    match st.state {
        Init => {
            // ファイルを閉じる
            loop {
                match st.src.drop() {
                    Ready(()) => {},
                    Pending => return Pending,
                }
            }
            // destも同じ
        }
        ReadWait => {
            // read__stack, src, destを非同期的に閉じる
            ...
        }
        ReadWait => {
            // write__stack, read__stack, src, destを非同期的に閉じる
            ...
        }
    }
}
```

つまり，その`yield`時点で有効な変数に対し`.drop()`を完了するまで呼び出し続けるってわけ．

こういう仕組みにすると`drop`を使って（非同期も含めた）キャンセルがキレイに書ける．
`copy`を行いたいが処理を3秒でタイムアウトしたいというケースは次のようになる．

```rust
let mut timeout__stack = complete_later(3s);
let mut copy__stack = copy(src, dest);

loop {
    // まずタイムアウトをチェック
    match timeout__stack.poll() {
        // タイムアウトした
        Ready(()) => {
            break;
        }
        Pending => {}
    }

    // 次にcopyが終わってるかチェック
    match copy__stack.poll() {
        Ready(()) => {
            break;
        }
        Pending => {
            // 両方とも進まないので処理を中断
            return Pending;
        }
    }
}

// ここでcopyのステートマシンをdropする．
// すると，内部でsrc, destについて処理の（非同期）キャンセルが自動で走ってそれを待機する
loop {
    match copy__stack.drop() {
        Ready(()) => {},
        Pending => return Pending,
    }
}
```

キャンセル = `drop`というのはRustのデザインの中でもかなり画期的だと思うんだが，
Rustにとって`async/await`は後知恵の第二級市民なので非同期キャンセルがうまく扱えないのでした．悲しいね