#pragma once

#include <condition_variable>
#include <mutex>
#include <optional>
#include <queue>

namespace bolt {

/**
 * A thread-safe queue implementation.
 *
 * @tparam T  the type of the elements within the queue
 */
template <typename T>
class blocking_queue
{
public:
    blocking_queue()
            : queue_()
            , mutex_()
            , cond_()
    {}

    ~blocking_queue() = default;

    /**
     * Add an element to the end of the queue.
     * @param t  the item to add to the queue
     */
    void enqueue(T t)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        queue_.push(t);
        cond_.notify_one();
    }

    /**
     * Removes the the element at the front of the queue.
     * If the queue is empty, this will block until there is an element available.
     */
    T dequeue_wait()
    {
        std::unique_lock<std::mutex> lock(mutex_);
        while(queue_.empty())
        {
            // release lock as long as the wait and reaquire it afterwards.
            cond_.wait(lock);
        }
        T val = queue_.front();
        queue_.pop();
        return val;
    }

    /**
    * Removes the the element at the front of the queue.
    * If the queue is empty, this will block until there is an element available.
    */
    std::optional<T> dequeue_nowait()
    {
        std::unique_lock<std::mutex> lock(mutex_);
        if (queue_.empty()) { return std::nullopt; }
        T val = queue_.front();
        queue_.pop();
        return val;
    }

private:
    std::queue<T> queue_;
    mutable std::mutex mutex_;
    std::condition_variable cond_;
};

}